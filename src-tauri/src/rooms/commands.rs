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

#[tauri::command]
pub async fn matrix_get_chats(
    auth_state: State<'_, AuthState>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatsResponse, String> {
    if let Some(cached_chats) = load_cached_chats(&app_handle)? {
        let _ = trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: None,
        });

        return Ok(MatrixGetChatsResponse {
            chats: cached_chats,
        });
    }

    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let local_chats = collect_chat_summaries(&client).await;
    if !local_chats.is_empty() {
        let _ = store_cached_chats(&app_handle, &local_chats);
        let _ = trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: None,
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
    })?;

    Ok(MatrixTriggerRoomUpdateResponse { queued: true })
}
