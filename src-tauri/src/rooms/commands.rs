use std::time::Duration;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::protocol::config;
use crate::protocol::sync::sync_once_serialized;

use super::persistence::{load_cached_chats, store_cached_chats};
use super::types::MatrixGetChatsResponse;
use super::workers::collect_chat_summaries;
use super::{
    MatrixTriggerRoomUpdateRequest, MatrixTriggerRoomUpdateResponse, RoomRefreshTrigger,
    RoomUpdateTriggerState,
};

fn has_stale_in_memory_chat_media(chats: &MatrixGetChatsResponse) -> bool {
    chats.chats.iter().any(|chat| {
        chat.image_url
            .as_deref()
            .is_some_and(|url| url.starts_with("matrix-media://"))
    })
}

#[tauri::command]
pub async fn matrix_get_chats(
    auth_state: State<'_, AuthState>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatsResponse, String> {
    if let Some(cached_chats) = load_cached_chats(&app_handle)? {
        let cached = MatrixGetChatsResponse {
            chats: cached_chats,
        };

        if has_stale_in_memory_chat_media(&cached) {
            let client = auth_state.restore_client_and_get(&app_handle).await?;

            let local_chats = collect_chat_summaries(&client).await;
            if !local_chats.is_empty() {
                let _ = store_cached_chats(&app_handle, &local_chats);
                let _ = trigger_state.enqueue(RoomRefreshTrigger {
                    selected_room_id: None,
                    include_selected_messages: false,
                });

                return Ok(MatrixGetChatsResponse { chats: local_chats });
            }
        }

        let _ = trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: None,
            include_selected_messages: false,
        });

        return Ok(cached);
    }

    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let local_chats = collect_chat_summaries(&client).await;
    if !local_chats.is_empty() {
        let _ = store_cached_chats(&app_handle, &local_chats);
        let _ = trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: None,
            include_selected_messages: false,
        });

        return Ok(MatrixGetChatsResponse { chats: local_chats });
    }

    sync_once_serialized(
        &client,
        matrix_sdk::config::SyncSettings::default()
            .timeout(Duration::from_secs(config::SYNC_TIMEOUT_SECONDS)),
    )
    .await
    .map_err(|error| format!("Failed to sync Matrix rooms: {error}"))?;

    let chats = collect_chat_summaries(&client).await;

    store_cached_chats(&app_handle, &chats)?;

    let _ = trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: None,
        include_selected_messages: false,
    });

    Ok(MatrixGetChatsResponse { chats })
}

#[tauri::command]
pub async fn matrix_trigger_room_update(
    request: Option<MatrixTriggerRoomUpdateRequest>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<MatrixTriggerRoomUpdateResponse, String> {
    let payload = request.unwrap_or_default();

    trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: payload.selected_room_id,
        include_selected_messages: payload.include_selected_messages,
    })?;

    Ok(MatrixTriggerRoomUpdateResponse { queued: true })
}

#[cfg(test)]
mod tests {
    use super::has_stale_in_memory_chat_media;
    use crate::rooms::types::{MatrixChatSummary, MatrixGetChatsResponse, MatrixRoomKind};

    fn chat_with_image(image_url: Option<&str>) -> MatrixChatSummary {
        MatrixChatSummary {
            room_id: String::from("!room:example.org"),
            display_name: String::from("Example"),
            image_url: image_url.map(ToOwned::to_owned),
            encrypted: false,
            joined_members: 2,
            kind: MatrixRoomKind::Room,
            joined: true,
            is_direct: false,
            parent_room_id: None,
        }
    }

    #[test]
    fn detects_stale_matrix_media_avatar_url() {
        let response = MatrixGetChatsResponse {
            chats: vec![chat_with_image(Some(
                "matrix-media://localhost/img-123.png",
            ))],
        };

        assert!(has_stale_in_memory_chat_media(&response));
    }

    #[test]
    fn ignores_non_stale_avatar_urls() {
        let response = MatrixGetChatsResponse {
            chats: vec![
                chat_with_image(None),
                chat_with_image(Some("asset://localhost/tmp/img-123.png")),
                chat_with_image(Some("https://example.org/avatar.png")),
            ],
        };

        assert!(!has_stale_in_memory_chat_media(&response));
    }
}
