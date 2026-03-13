use tauri::{AppHandle, State};

use crate::auth::AuthState;

use super::persistence::{load_cached_chats, store_cached_chats};
use super::types::MatrixGetChatsResponse;
use super::workers::{collect_chat_summaries, sync_client_rooms_once};
use super::{
    MatrixTriggerRoomUpdateRequest, MatrixTriggerRoomUpdateResponse, RoomRefreshTrigger,
    RoomUpdateTriggerState,
};

#[tauri::command]
pub async fn matrix_get_chats(
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatsResponse, String> {
    auth_state
        .restore_client_from_disk_if_needed(&app_handle)
        .await?;

    if let Some(cached_chats) = load_cached_chats(&app_handle)? {
        return Ok(MatrixGetChatsResponse {
            chats: cached_chats,
        });
    }

    let client = auth_state.client()?;
    sync_client_rooms_once(&client).await?;

    let chats = collect_chat_summaries(&client).await;

    store_cached_chats(&app_handle, &chats)?;

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
