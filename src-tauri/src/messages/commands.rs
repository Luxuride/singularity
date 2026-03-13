use log::info;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::rooms::{RoomRefreshTrigger, RoomUpdateTriggerState};
use crate::protocol::sync::sync_once_serialized;

use super::persistence::is_cacheable_initial_request;
use super::types::{MatrixGetChatMessagesRequest, MatrixGetChatMessagesResponse};
use super::workers::fetch_room_messages_from_client;
use super::MessageCacheState;

#[tauri::command]
pub async fn matrix_get_chat_messages(
    request: MatrixGetChatMessagesRequest,
    auth_state: State<'_, AuthState>,
    message_cache: State<'_, MessageCacheState>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatMessagesResponse, String> {
    info!("matrix_get_chat_messages requested");
    auth_state
        .restore_client_from_disk_if_needed(&app_handle)
        .await?;

    let from = request.from.clone();
    let cacheable_initial_request = is_cacheable_initial_request(from.as_deref(), request.limit);
    let limit = request.limit;
    let client = auth_state.client()?;

    if let Some(cached) = message_cache
        .load_initial_room_messages(request.room_id.as_str(), from.as_deref(), limit)
        .await
    {
        let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: Some(request.room_id.clone()),
        });
        return Ok(cached);
    }

    sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix room messages: {error}"))?;

    let response = fetch_room_messages_from_client(
        &client,
        request.room_id.as_str(),
        from,
        limit,
    )
    .await?;

    if cacheable_initial_request {
        message_cache.store_initial_room_messages(&response).await;
    }

    Ok(response)
}
