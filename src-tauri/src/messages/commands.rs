use log::info;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::protocol::sync::sync_once_serialized;

use super::types::{MatrixGetChatMessagesRequest, MatrixGetChatMessagesResponse};
use super::workers::fetch_room_messages_from_client;

#[tauri::command]
pub async fn matrix_get_chat_messages(
    request: MatrixGetChatMessagesRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatMessagesResponse, String> {
    info!("matrix_get_chat_messages requested");
    auth_state
        .restore_client_from_disk_if_needed(&app_handle)
        .await?;
    let client = auth_state.client()?;
    sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix room messages: {error}"))?;

    fetch_room_messages_from_client(
        &client,
        request.room_id.as_str(),
        request.from,
        request.limit,
    )
    .await
}
