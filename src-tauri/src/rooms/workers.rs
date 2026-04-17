use std::collections::{HashMap, HashSet};
use std::time::Duration;

use log::{error, info, warn};
use matrix_sdk::ruma::events::GlobalAccountDataEventType;
use matrix_sdk::ruma::events::StateEventType;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::auth::handle_unknown_token_error;
use crate::auth::AuthState;
use crate::db::AppDb;
use crate::messages::{
    fetch_room_messages_from_client, store_initial_room_messages, MatrixGetChatMessagesResponse,
};
use crate::protocol::config;

use super::persistence::{collect_and_store_chats, refresh_room_snapshot};
use super::types::{MatrixChatSummary, MatrixRoomKind};
use super::{
    MatrixRoomRemovedEvent, MatrixSelectedRoomMessagesEvent, RoomRefreshTrigger, RoomSnapshot,
    RoomUpdateEvent, RoomUpdateTriggerState,
};

pub(crate) async fn collect_chat_summaries(client: &matrix_sdk::Client) -> Vec<MatrixChatSummary> {
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

async fn direct_room_ids(client: &matrix_sdk::Client) -> HashSet<String> {
    let mut direct_room_ids = HashSet::new();
    let raw_content = match client
        .account()
        .account_data_raw(GlobalAccountDataEventType::from("m.direct"))
        .await
    {
        Ok(raw_content) => raw_content,
        Err(_) => return direct_room_ids,
    };

    let Some(raw_content) = raw_content else {
        return direct_room_ids;
    };

    let Ok(content) = raw_content.deserialize_as::<serde_json::Value>() else {
        return direct_room_ids;
    };

    let Some(mapping) = content.as_object() else {
        return direct_room_ids;
    };

    for room_ids in mapping.values() {
        let Some(room_ids) = room_ids.as_array() else {
            continue;
        };

        for room_id in room_ids {
            let Some(room_id) = room_id.as_str() else {
                continue;
            };

            if !room_id.is_empty() {
                direct_room_ids.insert(room_id.to_string());
            }
        }
    }

    direct_room_ids
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

pub fn start_room_update_worker(app: AppHandle) -> RoomUpdateTriggerState {
    let (sender, mut receiver) = mpsc::unbounded_channel::<RoomRefreshTrigger>();
    let task_app = app.clone();
    let initial_sync_timeout = Duration::from_secs(config::INITIAL_ROOM_SYNC_TIMEOUT_SECONDS);
    let long_poll_sync_timeout = Duration::from_secs(config::LONG_POLL_SYNC_TIMEOUT_SECONDS);
    let unauthenticated_delay = Duration::from_secs(config::WORKER_UNAUTH_SLEEP_SECONDS);
    let retry_initial_delay = Duration::from_millis(config::WORKER_RETRY_INITIAL_DELAY_MS);
    let startup_retry_max_delay = Duration::from_millis(config::WORKER_STARTUP_RETRY_MAX_DELAY_MS);
    let retry_max_delay = Duration::from_millis(config::WORKER_RETRY_MAX_DELAY_MS);

    tauri::async_runtime::spawn(async move {
        let mut previous_snapshot = RoomSnapshot::new();
        let mut selected_room_id = None::<String>;
        let mut include_selected_messages = false;
        let mut retry_delay = None::<Duration>;

        loop {
            while let Ok(trigger) = receiver.try_recv() {
                apply_trigger(
                    trigger,
                    &mut selected_room_id,
                    &mut include_selected_messages,
                );
            }

            match run_refresh_pass(
                &task_app,
                &mut previous_snapshot,
                selected_room_id.clone(),
                include_selected_messages,
                initial_sync_timeout,
                long_poll_sync_timeout,
            )
            .await
            {
                Ok(refresh_completed) => {
                    include_selected_messages = false;
                    retry_delay = None;

                    if !refresh_completed {
                        tokio::select! {
                            _ = tokio::time::sleep(unauthenticated_delay) => {}
                            maybe_trigger = receiver.recv() => {
                                let Some(trigger) = maybe_trigger else {
                                    break;
                                };
                                apply_trigger(
                                    trigger,
                                    &mut selected_room_id,
                                    &mut include_selected_messages,
                                );
                            }
                        }
                    }
                }
                Err(error) => {
                    include_selected_messages = false;
                    let has_snapshot = !previous_snapshot.is_empty();
                    if has_snapshot {
                        error!("Room update pass failed: {error}");
                    } else if is_transient_sync_timeout_error(&error) {
                        info!("Initial room sync timed out; retrying: {error}");
                    } else {
                        warn!("Initial room sync pass failed: {error}");
                    }

                    let max_retry_delay = if has_snapshot {
                        retry_max_delay
                    } else {
                        startup_retry_max_delay
                    };

                    let next_delay = retry_delay
                        .unwrap_or(retry_initial_delay)
                        .min(max_retry_delay);

                    retry_delay = Some(next_delay.saturating_mul(2).min(max_retry_delay));

                    tokio::select! {
                        _ = tokio::time::sleep(next_delay) => {}
                        maybe_trigger = receiver.recv() => {
                            let Some(trigger) = maybe_trigger else {
                                break;
                            };
                            apply_trigger(
                                trigger,
                                &mut selected_room_id,
                                &mut include_selected_messages,
                            );
                        }
                    }
                }
            }
        }
    });

    RoomUpdateTriggerState::new(sender)
}

async fn run_refresh_pass(
    app: &AppHandle,
    previous_snapshot: &mut RoomSnapshot,
    selected_room_id: Option<String>,
    include_selected_messages: bool,
    initial_sync_timeout: Duration,
    long_poll_sync_timeout: Duration,
) -> Result<bool, String> {
    let auth_state = app.state::<AuthState>();
    auth_state.restore_client_from_disk_if_needed(app).await?;

    let client = match auth_state.client() {
        Ok(client) => client,
        Err(_) => return Ok(false),
    };

    if previous_snapshot.is_empty() {
        let local_chats = collect_and_store_chats(app, &client).await;
        if !local_chats.is_empty() {
            let mut local_snapshot = RoomSnapshot::new();
            for chat in local_chats {
                local_snapshot.insert(chat.room_id.clone(), chat);
            }

            for chat in local_snapshot.values() {
                let _ = app.emit(RoomUpdateEvent::RoomAdded.as_str(), chat.clone());
            }

            *previous_snapshot = local_snapshot;
        }
    }

    let sync_timeout = if previous_snapshot.is_empty() {
        initial_sync_timeout
    } else {
        long_poll_sync_timeout
    };

    let current_snapshot = match refresh_room_snapshot(app, &client, sync_timeout).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if is_unknown_token_error(&error) {
                warn!("Room refresh failed with unknown token; clearing session");
                handle_unknown_token_error(app, &auth_state, &client).await?;
            } else {
                return Err(error);
            }
            return Ok(false);
        }
    };

    for (room_id, chat) in &current_snapshot {
        match previous_snapshot.get(room_id) {
            None => {
                let _ = app.emit(RoomUpdateEvent::RoomAdded.as_str(), chat.clone());
            }
            Some(previous) if previous != chat => {
                let _ = app.emit(RoomUpdateEvent::RoomUpdated.as_str(), chat.clone());
            }
            Some(_) => {}
        }
    }

    for room_id in previous_snapshot.keys() {
        if !current_snapshot.contains_key(room_id) {
            let _ = app.emit(
                RoomUpdateEvent::RoomRemoved.as_str(),
                MatrixRoomRemovedEvent {
                    room_id: room_id.clone(),
                },
            );
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

                    let app_db = app.state::<AppDb>();
                    if let Err(error) = store_initial_room_messages(&app_db, &response) {
                        warn!("Failed to persist selected-room message cache: {error}");
                    }

                    let _ = app.emit(
                        RoomUpdateEvent::SelectedRoomMessages.as_str(),
                        MatrixSelectedRoomMessagesEvent {
                            room_id: response.room_id,
                            next_from: response.next_from,
                            messages: response.messages,
                        },
                    );
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
