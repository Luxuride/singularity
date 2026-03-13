use log::info;
use std::time::Duration;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::protocol::config;

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
    client
        .sync_once(
            matrix_sdk::config::SyncSettings::default()
                .timeout(Duration::from_secs(config::SYNC_TIMEOUT_SECONDS)),
        )
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
