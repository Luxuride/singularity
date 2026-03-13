use std::time::Duration;

use log::{error, warn};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::auth::AuthState;
use crate::auth::handle_unknown_token_error;
use crate::messages::{fetch_room_messages_from_client, MessageCacheState};
use crate::protocol::config;
use crate::protocol::sync::sync_once_serialized;

use super::persistence::refresh_room_snapshot;
use super::types::MatrixChatSummary;
use super::{
    MatrixRoomRemovedEvent, MatrixSelectedRoomMessagesEvent, RoomRefreshTrigger, RoomSnapshot,
    RoomUpdateEvent, RoomUpdateTriggerState,
};

#[derive(Clone, Debug)]
struct RoomUpdateWorkerConfig {
    polling_interval: Duration,
}

impl Default for RoomUpdateWorkerConfig {
    fn default() -> Self {
        Self {
            polling_interval: Duration::from_secs(config::ROOM_UPDATE_POLL_INTERVAL_SECONDS),
        }
    }
}

pub(crate) async fn sync_client_rooms_once(client: &matrix_sdk::Client) -> Result<(), String> {
    sync_once_serialized(
        client,
        matrix_sdk::config::SyncSettings::default()
            .timeout(Duration::from_secs(config::SYNC_TIMEOUT_SECONDS)),
    )
    .await
    .map_err(|error| format!("Failed to sync Matrix rooms: {error}"))?;

    Ok(())
}

pub(crate) async fn collect_chat_summaries(client: &matrix_sdk::Client) -> Vec<MatrixChatSummary> {
    let mut chats = Vec::new();

    for room in client.joined_rooms() {
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

        chats.push(MatrixChatSummary {
            room_id: room.room_id().to_string(),
            display_name,
            encrypted,
            joined_members,
        });
    }

    chats.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    chats
}

pub fn start_room_update_worker(app: AppHandle) -> RoomUpdateTriggerState {
    let (sender, mut receiver) = mpsc::unbounded_channel::<RoomRefreshTrigger>();
    let task_app = app.clone();
    let worker_config = RoomUpdateWorkerConfig::default();

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(worker_config.polling_interval);
        let mut previous_snapshot = RoomSnapshot::new();

        loop {
            let mut trigger_room_id = None;

            tokio::select! {
                _ = interval.tick() => {}
                maybe_trigger = receiver.recv() => {
                    let Some(trigger) = maybe_trigger else {
                        break;
                    };

                    trigger_room_id = trigger.selected_room_id;

                    while let Ok(next_trigger) = receiver.try_recv() {
                        if next_trigger.selected_room_id.is_some() {
                            trigger_room_id = next_trigger.selected_room_id;
                        }
                    }
                }
            }

            if let Err(error) =
                run_refresh_pass(&task_app, &mut previous_snapshot, trigger_room_id).await
            {
                error!("Room update pass failed: {error}");
            }
        }
    });

    RoomUpdateTriggerState::new(sender)
}

async fn run_refresh_pass(
    app: &AppHandle,
    previous_snapshot: &mut RoomSnapshot,
    selected_room_id: Option<String>,
) -> Result<(), String> {
    let auth_state = app.state::<AuthState>();
    auth_state.restore_client_from_disk_if_needed(app).await?;

    let client = match auth_state.client() {
        Ok(client) => client,
        Err(_) => return Ok(()),
    };

    let current_snapshot = match refresh_room_snapshot(app, &client).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if is_unknown_token_error(&error) {
                warn!("Room refresh failed with unknown token; attempting token recovery");

                handle_unknown_token_error(app, &auth_state, &client).await?;

                // Retry once after refresh so restart-time token expiry doesn't force a logout.
                match refresh_room_snapshot(app, &client).await {
                    Ok(snapshot) => snapshot,
                    Err(retry_error) => {
                        if is_unknown_token_error(&retry_error) {
                            warn!(
                                "Room refresh still failing with unknown token after refresh; clearing local session"
                            );
                            auth_state.clear_session_everywhere(app)?;
                            return Ok(());
                        }

                        return Err(retry_error);
                    }
                }
            } else {
                return Err(error);
            }
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

    if let Some(room_id) = selected_room_id {
        if current_snapshot.contains_key(&room_id) {
            if let Ok(response) =
                fetch_room_messages_from_client(&client, &room_id, None, Some(50)).await
            {
                let message_cache = app.state::<MessageCacheState>();
                message_cache.store_initial_room_messages(&response).await;

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

    *previous_snapshot = current_snapshot;
    Ok(())
}

fn is_unknown_token_error(error: &str) -> bool {
    error.contains("M_UNKNOWN_TOKEN") || error.contains("refresh token does not exist")
}
