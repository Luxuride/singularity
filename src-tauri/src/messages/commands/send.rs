use log::info;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::protocol::sync::sync_once_serialized;
use crate::rooms::{RoomRefreshTrigger, RoomUpdateTriggerState};

use super::super::emoji::load_picker_assets_from_client;
use super::super::send::{
    build_formatted_body_from_custom_emoji_for_send, send_room_message_from_client,
};
use super::super::types::{MatrixSendChatMessageRequest, MatrixSendChatMessageResponse};

#[tauri::command]
pub async fn matrix_send_chat_message(
    request: MatrixSendChatMessageRequest,
    auth_state: State<'_, AuthState>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixSendChatMessageResponse, String> {
    info!("matrix_send_chat_message requested");
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix before send: {error}"))?;

    let room_id = request.room_id;
    let picker_custom_emoji = load_picker_assets_from_client(&client).await?;
    let formatted_body = build_formatted_body_from_custom_emoji_for_send(
        request.body.as_str(),
        &picker_custom_emoji,
    );
    let event_id = send_room_message_from_client(
        &client,
        room_id.as_str(),
        request.body.as_str(),
        &picker_custom_emoji,
    )
    .await?;

    let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: Some(room_id),
        include_selected_messages: false,
    });

    Ok(MatrixSendChatMessageResponse {
        event_id,
        formatted_body,
    })
}
