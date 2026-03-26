use std::collections::{HashMap, HashSet};
use std::time::Duration;

use futures_util::StreamExt;
use log::{error, warn};
use matrix_sdk::ruma::events::StateEventType;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::auth::handle_unknown_token_error;
use crate::auth::AuthState;
use crate::db::AppDb;
use crate::messages::{
    cache_mxc_media_to_local_path, fetch_room_messages_from_client, store_initial_room_messages,
};
use crate::protocol::config;

use super::persistence::refresh_room_snapshot;
use super::types::{MatrixChatSummary, MatrixRoomKind};
use super::{
    MatrixRoomRemovedEvent, MatrixSelectedRoomMessagesEvent, RoomRefreshTrigger, RoomSnapshot,
    RoomUpdateEvent, RoomUpdateTriggerState,
};

pub(crate) async fn collect_chat_summaries(client: &matrix_sdk::Client) -> Vec<MatrixChatSummary> {
    let joined_rooms = client.joined_rooms();
    let linked_parent_space_ids_by_child =
        linked_parent_space_ids_by_child_room(&joined_rooms).await;
    let mut chats = Vec::new();
    let joined_room_ids = joined_rooms
        .iter()
        .map(|room| room.room_id().to_string())
        .collect::<HashSet<_>>();
    let mut unjoined_parent_space_ids = HashSet::new();

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
        let is_direct = room.is_direct().await.unwrap_or(false);
        let kind = if room.is_space() {
            MatrixRoomKind::Space
        } else {
            MatrixRoomKind::Room
        };
        let mut parent_room_ids = parent_space_ids(&room).await;
        if let Some(linked_parent_ids) =
            linked_parent_space_ids_by_child.get(room.room_id().as_str())
        {
            for linked_parent_id in linked_parent_ids {
                if !parent_room_ids.contains(linked_parent_id) {
                    parent_room_ids.push(linked_parent_id.clone());
                }
            }
        }
        let parent_room_id = parent_room_ids.first().cloned();
        let room_avatar_image_url = match room.avatar_url() {
            Some(mxc) => cache_mxc_media_to_local_path(client, mxc.as_str()).await,
            None => None,
        };
        let image_url = if is_direct && matches!(kind, MatrixRoomKind::Room) {
            match room_avatar_image_url {
                Some(room_avatar) => Some(room_avatar),
                None => direct_room_counterparty_avatar_url(client, &room).await,
            }
        } else {
            room_avatar_image_url
        };

        for parent_id in &parent_room_ids {
            if !joined_room_ids.contains(parent_id) {
                unjoined_parent_space_ids.insert(parent_id.clone());
            }
        }

        chats.push(MatrixChatSummary {
            room_id: room.room_id().to_string(),
            display_name,
            image_url,
            encrypted,
            joined_members,
            kind,
            joined: true,
            is_direct,
            parent_room_id,
            parent_room_ids,
        });
    }

    for space_id in unjoined_parent_space_ids {
        chats.push(MatrixChatSummary {
            room_id: space_id.clone(),
            display_name: space_id,
            image_url: None,
            encrypted: false,
            joined_members: 0,
            kind: MatrixRoomKind::Space,
            joined: false,
            is_direct: false,
            parent_room_id: None,
            parent_room_ids: vec![],
        });
    }

    chats.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    chats
}

async fn linked_parent_space_ids_by_child_room(
    joined_rooms: &[matrix_sdk::Room],
) -> HashMap<String, Vec<String>> {
    let mut linked_parents_by_child = HashMap::<String, HashSet<String>>::new();

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

            linked_parents_by_child
                .entry(child_room_id.to_string())
                .or_default()
                .insert(parent_space_id.clone());
        }
    }

    linked_parents_by_child
        .into_iter()
        .map(|(child_room_id, parent_ids)| {
            let mut parent_ids = parent_ids.into_iter().collect::<Vec<_>>();
            parent_ids.sort();
            (child_room_id, parent_ids)
        })
        .collect()
}

async fn direct_room_counterparty_avatar_url(
    client: &matrix_sdk::Client,
    room: &matrix_sdk::Room,
) -> Option<String> {
    let own_user_id = client.user_id()?.to_owned();
    let members = room
        .members(matrix_sdk::RoomMemberships::JOIN | matrix_sdk::RoomMemberships::INVITE)
        .await
        .ok()?;

    for member in members {
        if member.user_id().as_str() == own_user_id.as_str() {
            continue;
        }

        let avatar_url = member.avatar_url()?;
        return cache_mxc_media_to_local_path(client, avatar_url.as_str()).await;
    }

    None
}

async fn parent_space_ids(room: &matrix_sdk::Room) -> Vec<String> {
    let mut parent_spaces = match room.parent_spaces().await {
        Ok(parent_spaces) => parent_spaces,
        Err(_) => return vec![],
    };

    let mut candidates = Vec::<(u8, String)>::new();

    while let Some(parent_result) = parent_spaces.next().await {
        let Ok(parent_space) = parent_result else {
            continue;
        };

        let candidate = match parent_space {
            matrix_sdk::room::ParentSpace::Reciprocal(parent_room) => {
                Some((0_u8, parent_room.room_id().to_string()))
            }
            matrix_sdk::room::ParentSpace::WithPowerlevel(parent_room) => {
                Some((1_u8, parent_room.room_id().to_string()))
            }
            matrix_sdk::room::ParentSpace::Unverifiable(parent_room_id) => {
                Some((2_u8, parent_room_id.to_string()))
            }
            matrix_sdk::room::ParentSpace::Illegitimate(_) => None,
        };

        let Some((rank, parent_room_id)) = candidate else {
            continue;
        };

        candidates.push((rank, parent_room_id));
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut unique_parent_ids = Vec::<String>::new();
    for (_, parent_room_id) in candidates {
        if !unique_parent_ids.contains(&parent_room_id) {
            unique_parent_ids.push(parent_room_id);
        }
    }

    unique_parent_ids
}

pub fn start_room_update_worker(app: AppHandle) -> RoomUpdateTriggerState {
    let (sender, mut receiver) = mpsc::unbounded_channel::<RoomRefreshTrigger>();
    let task_app = app.clone();
    let sync_timeout = Duration::from_secs(config::LONG_POLL_SYNC_TIMEOUT_SECONDS);
    let unauthenticated_delay = Duration::from_secs(config::WORKER_UNAUTH_SLEEP_SECONDS);
    let retry_initial_delay = Duration::from_millis(config::WORKER_RETRY_INITIAL_DELAY_MS);
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
                sync_timeout,
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
                    error!("Room update pass failed: {error}");

                    let next_delay = retry_delay
                        .unwrap_or(retry_initial_delay)
                        .min(retry_max_delay);

                    retry_delay = Some(next_delay.saturating_mul(2).min(retry_max_delay));

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
    sync_timeout: Duration,
) -> Result<bool, String> {
    let auth_state = app.state::<AuthState>();
    auth_state.restore_client_from_disk_if_needed(app).await?;

    let client = match auth_state.client() {
        Ok(client) => client,
        Err(_) => return Ok(false),
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
