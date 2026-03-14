use std::time::Duration;

use log::{error, warn};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::auth::AuthState;
use crate::auth::handle_unknown_token_error;
use crate::messages::{fetch_room_messages_from_client, MessageCacheState};
use crate::protocol::config;

use super::persistence::refresh_room_snapshot;
use super::types::MatrixChatSummary;
use super::{
    MatrixRoomRemovedEvent, MatrixSelectedRoomMessagesEvent, RoomRefreshTrigger, RoomSnapshot,
    RoomUpdateEvent, RoomUpdateTriggerState,
};

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
    let sync_timeout = Duration::from_secs(config::LONG_POLL_SYNC_TIMEOUT_SECONDS);
    let unauthenticated_delay = Duration::from_secs(config::WORKER_UNAUTH_SLEEP_SECONDS);
    let retry_initial_delay = Duration::from_millis(config::WORKER_RETRY_INITIAL_DELAY_MS);
    let retry_max_delay = Duration::from_millis(config::WORKER_RETRY_MAX_DELAY_MS);

    tauri::async_runtime::spawn(async move {
        let mut previous_snapshot = RoomSnapshot::new();
        let mut selected_room_id = None::<String>;
        let mut retry_delay = None::<Duration>;

        loop {
            while let Ok(trigger) = receiver.try_recv() {
                if let Some(room_id) = trigger.selected_room_id {
                    if room_id.is_empty() {
                        selected_room_id = None;
                    } else {
                        selected_room_id = Some(room_id);
                    }
                }
            }

            match run_refresh_pass(
                &task_app,
                &mut previous_snapshot,
                selected_room_id.clone(),
                sync_timeout,
            )
            .await
            {
                Ok(refresh_completed) => {
                    retry_delay = None;

                    if !refresh_completed {
                        tokio::select! {
                            _ = tokio::time::sleep(unauthenticated_delay) => {}
                            maybe_trigger = receiver.recv() => {
                                let Some(trigger) = maybe_trigger else {
                                    break;
                                };

                                if let Some(room_id) = trigger.selected_room_id {
                                    if room_id.is_empty() {
                                        selected_room_id = None;
                                    } else {
                                        selected_room_id = Some(room_id);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    error!("Room update pass failed: {error}");

                    let next_delay = retry_delay
                        .unwrap_or(retry_initial_delay)
                        .min(retry_max_delay);

                    retry_delay = Some(
                        next_delay
                            .saturating_mul(2)
                            .min(retry_max_delay),
                    );

                    tokio::select! {
                        _ = tokio::time::sleep(next_delay) => {}
                        maybe_trigger = receiver.recv() => {
                            let Some(trigger) = maybe_trigger else {
                                break;
                            };

                            if let Some(room_id) = trigger.selected_room_id {
                                if room_id.is_empty() {
                                    selected_room_id = None;
                                } else {
                                    selected_room_id = Some(room_id);
                                }
                            }
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
                warn!("Room refresh failed with unknown token; attempting token recovery");

                let recovered = handle_unknown_token_error(app, &auth_state, &client).await?;

                if !recovered {
                    return Ok(false);
                }

                // Retry once after refresh so restart-time token expiry doesn't force a logout.
                match refresh_room_snapshot(app, &client, sync_timeout).await {
                    Ok(snapshot) => snapshot,
                    Err(retry_error) => {
                        if is_unknown_token_error(&retry_error) {
                            warn!(
                                "Room refresh still failing with unknown token after refresh; clearing local session"
                            );
                            auth_state.clear_session_everywhere(app)?;
                            return Ok(false);
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
    Ok(true)
}

fn is_unknown_token_error(error: &str) -> bool {
    error.contains("M_UNKNOWN_TOKEN")
        || error.contains("refresh token does not exist")
        || error.contains("refresh token isn't valid anymore")
}
