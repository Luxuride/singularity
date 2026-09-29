use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use log::{error, info, warn};
use matrix_sdk::ruma::events::StateEventType;
use tokio::sync::mpsc;

use auth::handle_unknown_token_error;
use auth::AuthState;
use chat::fetch_room_messages_from_client;
use chat::store_initial_room_messages;
use storage::AppDb;
use types::chat::MatrixGetChatMessagesResponse;
use types::config;
use types::event_paths;
use types::rooms::{
    MatrixChatSummary, MatrixRoomKind, MatrixRoomRemovedEvent, MatrixSelectedRoomMessagesEvent,
};
use types::{EventSink, Paths, RoomRefreshTrigger, RoomUpdateTriggerState};

use crate::direct::direct_room_ids;
use crate::persistence::{collect_and_store_chats, refresh_room_snapshot};

pub type RoomSnapshot = HashMap<String, MatrixChatSummary>;

pub async fn collect_chat_summaries(client: &matrix_sdk::Client) -> Vec<MatrixChatSummary> {
    let joined_rooms = client.joined_rooms();
    let children_by_parent = children_room_ids_by_parent_room(&joined_rooms).await;
    let direct_room_ids = direct_room_ids(client).await;
    let mut chats = Vec::with_capacity(joined_rooms.len());

    for room in joined_rooms {
        let display_name = room
            .display_name()
            .await
            .map(|name| name.to_string())
            .unwrap_or_else(|_| room.room_id().to_string());

        let encrypted = room
            .latest_encryption_state()
            .await
            .map(|state| state.is_encrypted())
            .unwrap_or(false);
        let joined_members = room.joined_members_count();
        let is_direct = direct_room_ids.contains(room.room_id().as_str());
        let kind = if room.is_space() {
            MatrixRoomKind::Space
        } else {
            MatrixRoomKind::Room
        };
        let children_room_ids = children_by_parent
            .get(room.room_id().as_str())
            .cloned()
            .unwrap_or_default();

        chats.push(MatrixChatSummary {
            room_id: room.room_id().to_string(),
            display_name,
            image_url: None,
            encrypted,
            joined_members,
            kind,
            joined: true,
            is_direct,
            children_room_ids,
        });
    }

    chats.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });

    chats
}

async fn children_room_ids_by_parent_room(
    joined_rooms: &[matrix_sdk::room::Room],
) -> HashMap<String, Vec<String>> {
    let mut children_by_parent = HashMap::<String, HashSet<String>>::new();

    for room in joined_rooms {
        if !room.is_space() {
            continue;
        }

        let parent_space_id = room.room_id().to_string();
        let state_events = match room
            .get_state_events(StateEventType::from("m.space.child"))
            .await
        {
            Ok(state_events) => state_events,
            Err(_) => continue,
        };

        for raw_event in state_events {
            let Ok(event) = serde_json::to_value(&raw_event) else {
                continue;
            };

            let Some(child_room_id) = event.get("state_key").and_then(|value| value.as_str())
            else {
                continue;
            };

            if child_room_id.is_empty() {
                continue;
            }

            children_by_parent
                .entry(parent_space_id.clone())
                .or_default()
                .insert(child_room_id.to_string());
        }
    }

    children_by_parent
        .into_iter()
        .map(|(parent_room_id, child_ids)| {
            let mut child_ids = child_ids.into_iter().collect::<Vec<_>>();
            child_ids.sort();
            (parent_room_id, child_ids)
        })
        .collect()
}

/// The background room-update worker. Holds the receiver side of the trigger
/// channel plus the shared state needed to run a refresh pass.
///
/// This crate is Tauri-free and cannot assume a Tokio runtime is running, so it
/// does not spawn the loop itself. The binder spawns [`RoomUpdateWorker::run`]
/// on its async runtime (e.g. Tauri's managed runtime).
pub struct RoomUpdateWorker {
    receiver: mpsc::UnboundedReceiver<RoomRefreshTrigger>,
    paths: Paths,
    app_db: Arc<AppDb>,
    auth_state: Arc<AuthState>,
    event_sink: Arc<dyn EventSink>,
}

impl RoomUpdateWorker {
    /// Run the worker loop to completion. Spawn this on an async runtime.
    pub async fn run(mut self) {
        let initial_sync_timeout = Duration::from_secs(config::INITIAL_ROOM_SYNC_TIMEOUT_SECONDS);
        let long_poll_sync_timeout = Duration::from_secs(config::LONG_POLL_SYNC_TIMEOUT_SECONDS);
        let unauthenticated_delay = Duration::from_secs(config::WORKER_UNAUTH_SLEEP_SECONDS);
        let retry_initial_delay = Duration::from_millis(config::WORKER_RETRY_INITIAL_DELAY_MS);
        let startup_retry_max_delay =
            Duration::from_millis(config::WORKER_STARTUP_RETRY_MAX_DELAY_MS);
        let retry_max_delay = Duration::from_millis(config::WORKER_RETRY_MAX_DELAY_MS);

        let mut previous_snapshot = RoomSnapshot::new();
        let mut selected_room_id = None::<String>;
        let mut include_selected_messages = false;
        let mut retry_delay = None::<Duration>;
        let mut pending_trigger = None::<RoomRefreshTrigger>;

        loop {
            if let Some(trigger) = pending_trigger.take() {
                apply_trigger(
                    trigger,
                    &mut selected_room_id,
                    &mut include_selected_messages,
                );
            }

            while let Ok(trigger) = self.receiver.try_recv() {
                apply_trigger(
                    trigger,
                    &mut selected_room_id,
                    &mut include_selected_messages,
                );
            }

            // A refresh pass ends in a long poll that holds the per-homeserver
            // sync lock for as long as the server keeps the request open, and
            // every send and reaction queues behind that lock. A new trigger
            // means the answer has already changed, so the in-flight poll is
            // cancelled rather than waited out: the next pass re-syncs with the
            // shorter initial timeout and picks up the new state.
            //
            // Read before the pass borrows the snapshot.
            let had_snapshot = !previous_snapshot.is_empty();

            let interrupted = {
                // The pass borrows the loop's locals rather than `self`, because a
                // pinned future keeps its borrows alive for the whole `select` and
                // would otherwise freeze the receiver too. The block ends before
                // the snapshot is touched again, releasing that borrow.
                let deps = RefreshDeps {
                    app_db: &self.app_db,
                    auth_state: &self.auth_state,
                    event_sink: &self.event_sink,
                    paths: &self.paths,
                };
                let receiver = &mut self.receiver;
                let pass = run_refresh_pass(
                    &deps,
                    &mut previous_snapshot,
                    selected_room_id.clone(),
                    include_selected_messages,
                    initial_sync_timeout,
                    long_poll_sync_timeout,
                );
                tokio::pin!(pass);

                tokio::select! {
                    result = &mut pass => match result {
                        Ok(refresh_completed) => {
                            include_selected_messages = false;
                            retry_delay = None;

                            if !refresh_completed
                                && !drain_triggers_and_wait(
                                    receiver,
                                    unauthenticated_delay,
                                    &mut selected_room_id,
                                    &mut include_selected_messages,
                                )
                                .await
                            {
                                break;
                            }

                            false
                        }
                        Err(error) => {
                            include_selected_messages = false;
                            if had_snapshot {
                                error!("Room update pass failed: {error}");
                            } else if is_transient_sync_timeout_error(&error) {
                                info!("Initial room sync timed out; retrying: {error}");
                            } else {
                                warn!("Initial room sync pass failed: {error}");
                            }

                            let max_retry_delay = if had_snapshot {
                                retry_max_delay
                            } else {
                                startup_retry_max_delay
                            };

                            let next_delay = retry_delay
                                .unwrap_or(retry_initial_delay)
                                .min(max_retry_delay);

                            retry_delay = Some(next_delay.saturating_mul(2).min(max_retry_delay));

                            if !drain_triggers_and_wait(
                                receiver,
                                next_delay,
                                &mut selected_room_id,
                                &mut include_selected_messages,
                            )
                            .await
                            {
                                break;
                            }

                            false
                        }
                    },
                    trigger = receiver.recv() => {
                        match trigger {
                            Some(trigger) => pending_trigger = Some(trigger),
                            None => break,
                        }

                        true
                    }
                }
            };

            // An interrupted pass leaves the snapshot half-applied, and a
            // pass that emitted room-added events before being cancelled would
            // re-emit them next time. Rebuilding from the stored cache keeps the
            // diff against the next complete snapshot meaningful.
            if interrupted {
                include_selected_messages = false;
                previous_snapshot = RoomSnapshot::new();

                if let Ok(client) = self.auth_state.client() {
                    for chat in collect_and_store_chats(&self.app_db, &client).await {
                        previous_snapshot.insert(chat.room_id.clone(), chat);
                    }
                }
            }
        }
    }
}

/// The shared state a refresh pass reads. Split out of [`RoomUpdateWorker`] so
/// the pass's future borrows nothing the loop needs to mutate.
struct RefreshDeps<'a> {
    app_db: &'a Arc<AppDb>,
    auth_state: &'a Arc<AuthState>,
    event_sink: &'a Arc<dyn EventSink>,
    paths: &'a Paths,
}

/// Create the trigger channel and the worker. The binder manages the returned
/// [`RoomUpdateTriggerState`] and spawns [`RoomUpdateWorker::run`] on its async
/// runtime.
pub fn start_room_update_worker(
    paths: &Paths,
    app_db: Arc<AppDb>,
    auth_state: Arc<AuthState>,
    event_sink: Arc<dyn EventSink>,
) -> (RoomUpdateTriggerState, RoomUpdateWorker) {
    let (sender, receiver) = mpsc::unbounded_channel::<RoomRefreshTrigger>();
    let worker = RoomUpdateWorker {
        receiver,
        paths: paths.clone(),
        app_db,
        auth_state,
        event_sink,
    };
    (RoomUpdateTriggerState::new(sender), worker)
}

async fn run_refresh_pass(
    worker: &RefreshDeps<'_>,
    previous_snapshot: &mut RoomSnapshot,
    selected_room_id: Option<String>,
    include_selected_messages: bool,
    initial_sync_timeout: Duration,
    long_poll_sync_timeout: Duration,
) -> Result<bool, String> {
    let paths = &worker.paths;
    let app_db = &worker.app_db;
    let auth_state = &worker.auth_state;
    let event_sink = &worker.event_sink;

    auth_state
        .restore_client_from_disk_if_needed(paths, app_db)
        .await?;

    let client = match auth_state.client() {
        Ok(client) => client,
        Err(_) => return Ok(false),
    };

    if previous_snapshot.is_empty() {
        let local_chats = collect_and_store_chats(app_db, &client).await;
        if !local_chats.is_empty() {
            let mut local_snapshot = RoomSnapshot::new();
            for chat in local_chats {
                local_snapshot.insert(chat.room_id.clone(), chat);
            }

            for chat in local_snapshot.values() {
                let payload = serde_json::to_value(chat)
                    .map_err(|error| format!("Failed to serialize room: {error}"))?;
                let _ = event_sink.emit(event_paths::ROOM_ADDED, &payload);
            }

            *previous_snapshot = local_snapshot;
        }
    }

    let sync_timeout = if previous_snapshot.is_empty() {
        initial_sync_timeout
    } else {
        long_poll_sync_timeout
    };

    let current_snapshot = match refresh_room_snapshot(app_db, &client, sync_timeout).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if is_unknown_token_error(&error) {
                warn!("Room refresh failed with unknown token; clearing session");
                handle_unknown_token_error(paths, app_db, auth_state, &client).await?;
            } else {
                return Err(error);
            }
            return Ok(false);
        }
    };

    for (room_id, chat) in &current_snapshot {
        match previous_snapshot.get(room_id) {
            None => {
                let payload = serde_json::to_value(chat)
                    .map_err(|error| format!("Failed to serialize room: {error}"))?;
                let _ = event_sink.emit(event_paths::ROOM_ADDED, &payload);
            }
            Some(previous) if previous != chat => {
                let payload = serde_json::to_value(chat)
                    .map_err(|error| format!("Failed to serialize room: {error}"))?;
                let _ = event_sink.emit(event_paths::ROOM_UPDATED, &payload);
            }
            Some(_) => {}
        }
    }

    for room_id in previous_snapshot.keys() {
        if !current_snapshot.contains_key(room_id) {
            let payload = serde_json::to_value(MatrixRoomRemovedEvent {
                room_id: room_id.clone(),
            })
            .map_err(|error| format!("Failed to serialize room-removed event: {error}"))?;
            let _ = event_sink.emit(event_paths::ROOM_REMOVED, &payload);
        }
    }

    if include_selected_messages {
        if let Some(room_id) = selected_room_id {
            if current_snapshot.contains_key(&room_id) {
                if let Ok(response) =
                    fetch_room_messages_from_client(&client, &room_id, None, Some(50)).await
                {
                    let response = MatrixGetChatMessagesResponse {
                        room_id: response.room_id,
                        next_from: response.next_from,
                        messages: response.messages.into_iter().rev().collect(),
                    };

                    if let Err(error) = store_initial_room_messages(app_db, &response).await {
                        warn!("Failed to persist selected-room message cache: {error}");
                    }

                    let payload = serde_json::to_value(MatrixSelectedRoomMessagesEvent {
                        room_id: response.room_id,
                        next_from: response.next_from,
                        messages: response.messages,
                    })
                    .map_err(|error| {
                        format!("Failed to serialize selected-room messages: {error}")
                    })?;
                    let _ = event_sink.emit(event_paths::SELECTED_ROOM_MESSAGES, &payload);
                }
            }
        }
    }

    *previous_snapshot = current_snapshot;
    Ok(true)
}

fn is_unknown_token_error(error: &str) -> bool {
    error.contains("M_UNKNOWN_TOKEN")
        || error.contains("refresh token does not exist")
        || error.contains("refresh token isn't valid anymore")
}

fn is_transient_sync_timeout_error(error: &str) -> bool {
    error.contains("error sending request")
        || error.contains("timed out")
        || error.contains("deadline has elapsed")
}

fn apply_trigger(
    trigger: RoomRefreshTrigger,
    selected: &mut Option<String>,
    include_selected_messages: &mut bool,
) {
    if let Some(room_id) = trigger.selected_room_id {
        *selected = if room_id.is_empty() {
            None
        } else {
            Some(room_id)
        };
    }

    if trigger.include_selected_messages {
        *include_selected_messages = true;
    }
}

/// Wait for a trigger or a delay, applying any received trigger to the worker
/// state. Returns `false` when the channel closed (worker should stop).
async fn drain_triggers_and_wait(
    receiver: &mut mpsc::UnboundedReceiver<RoomRefreshTrigger>,
    delay: Duration,
    selected_room_id: &mut Option<String>,
    include_selected_messages: &mut bool,
) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => true,
        maybe_trigger = receiver.recv() => {
            let Some(trigger) = maybe_trigger else {
                return false;
            };
            apply_trigger(trigger, selected_room_id, include_selected_messages);
            true
        }
    }
}
