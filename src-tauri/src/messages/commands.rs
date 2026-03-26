use log::info;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::db::AppDb;
use crate::protocol::sync::sync_once_serialized;
use crate::rooms::{RoomRefreshTrigger, RoomUpdateTriggerState};

use super::emoji::load_picker_assets_from_client;
use super::persistence::{
    is_cacheable_initial_request, load_initial_room_messages, store_initial_room_messages,
};
use super::reactions::toggle_reaction_from_client;
use super::receive::{
    fetch_room_messages_from_client, stream_room_messages_from_client, StreamRoomMessagesContext,
};
use super::send::send_room_message_from_client;
use super::types::{
    MatrixGetChatMessagesRequest, MatrixGetChatMessagesResponse, MatrixGetEmojiPacksResponse,
    MatrixSendChatMessageRequest, MatrixSendChatMessageResponse, MatrixStreamChatMessagesRequest,
    MatrixStreamChatMessagesResponse, MatrixToggleReactionRequest, MatrixToggleReactionResponse,
};

fn has_stale_in_memory_media_urls(response: &MatrixGetChatMessagesResponse) -> bool {
    response
        .messages
        .iter()
        .any(|message| {
            message
                .image_url
                .as_deref()
                .is_some_and(|url| url.starts_with("matrix-media://"))
        })
}

fn is_room_unavailable_error(error: &str) -> bool {
    error.contains("Room is not available in current session")
}

#[tauri::command]
pub async fn matrix_get_emoji_packs(
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetEmojiPacksResponse, String> {
    info!("matrix_get_emoji_packs requested");
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix before loading emoji packs: {error}"))?;

    let custom_emoji = load_picker_assets_from_client(&client).await?;

    Ok(MatrixGetEmojiPacksResponse { custom_emoji })
}

#[tauri::command]
pub async fn matrix_get_chat_messages(
    request: MatrixGetChatMessagesRequest,
    auth_state: State<'_, AuthState>,
    app_db: State<'_, AppDb>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatMessagesResponse, String> {
    info!("matrix_get_chat_messages requested");
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let from = request.from.clone();
    let cacheable_initial_request = is_cacheable_initial_request(from.as_deref(), request.limit);
    let limit = request.limit;

    if let Some(cached) =
        load_initial_room_messages(&app_db, request.room_id.as_str(), from.as_deref(), limit)?
    {
        let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: Some(request.room_id.clone()),
            include_selected_messages: true,
        });

        if !has_stale_in_memory_media_urls(&cached) {
            return Ok(cached);
        }
    }

    let response = match fetch_room_messages_from_client(
        &client,
        request.room_id.as_str(),
        from.clone(),
        limit,
    )
    .await
    {
        Ok(response) => response,
        Err(error) if is_room_unavailable_error(&error) => {
            sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
                .await
                .map_err(|sync_error| {
                    format!(
                        "Failed to sync Matrix room messages after room-unavailable error: {sync_error}"
                    )
                })?;

            fetch_room_messages_from_client(&client, request.room_id.as_str(), from, limit).await?
        }
        Err(error) => return Err(error),
    };

    if cacheable_initial_request {
        store_initial_room_messages(&app_db, &response)?;
    }

    Ok(response)
}

#[tauri::command]
pub async fn matrix_stream_chat_messages(
    request: MatrixStreamChatMessagesRequest,
    auth_state: State<'_, AuthState>,
    app_db: State<'_, AppDb>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixStreamChatMessagesResponse, String> {
    info!("matrix_stream_chat_messages requested");
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let stream_result = stream_room_messages_from_client(
        StreamRoomMessagesContext {
            app_handle: &app_handle,
            app_db: &app_db,
            room_update_trigger_state: &room_update_trigger_state,
            client: &client,
        },
        request.clone(),
    )
    .await;

    match stream_result {
        Ok(response) => Ok(response),
        Err(error) if is_room_unavailable_error(&error) => {
            sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
                .await
                .map_err(|sync_error| {
                    format!(
                        "Failed to sync Matrix stream after room-unavailable error: {sync_error}"
                    )
                })?;

            stream_room_messages_from_client(
                StreamRoomMessagesContext {
                    app_handle: &app_handle,
                    app_db: &app_db,
                    room_update_trigger_state: &room_update_trigger_state,
                    client: &client,
                },
                request,
            )
            .await
        }
        Err(error) => Err(error),
    }
}

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
    let event_id = send_room_message_from_client(
        &client,
        room_id.as_str(),
        request.body.as_str(),
        request.formatted_body.as_deref(),
    )
    .await?;

    let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: Some(room_id),
        include_selected_messages: false,
    });

    Ok(MatrixSendChatMessageResponse { event_id })
}

#[tauri::command]
pub async fn matrix_toggle_reaction(
    request: MatrixToggleReactionRequest,
    auth_state: State<'_, AuthState>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixToggleReactionResponse, String> {
    info!("matrix_toggle_reaction requested");
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix before reaction toggle: {error}"))?;

    let room_id = request.room_id;
    let (added, event_id) = toggle_reaction_from_client(
        &client,
        room_id.as_str(),
        request.target_event_id.as_str(),
        request.key.as_str(),
    )
    .await?;

    let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: Some(room_id),
        include_selected_messages: true,
    });

    Ok(MatrixToggleReactionResponse { added, event_id })
}

#[cfg(test)]
mod tests {
    use super::has_stale_in_memory_media_urls;
    use crate::messages::types::{
        MatrixChatMessage, MatrixGetChatMessagesResponse, MatrixMessageDecryptionStatus,
        MatrixMessageVerificationStatus,
    };

    fn message_with_image(image_url: Option<&str>) -> MatrixChatMessage {
        MatrixChatMessage {
            event_id: Some(String::from("$event")),
            sender: String::from("@alice:example.org"),
            timestamp: Some(1),
            body: String::from("body"),
            formatted_body: None,
            message_type: Some(String::from("m.image")),
            image_url: image_url.map(ToOwned::to_owned),
            custom_emojis: Vec::new(),
            reactions: Vec::new(),
            encrypted: false,
            decryption_status: MatrixMessageDecryptionStatus::Plaintext,
            verification_status: MatrixMessageVerificationStatus::Unknown,
        }
    }

    #[test]
    fn detects_stale_matrix_media_url() {
        let response = MatrixGetChatMessagesResponse {
            room_id: String::from("!room:example.org"),
            next_from: None,
            messages: vec![message_with_image(Some("matrix-media://localhost/img-123.png"))],
        };

        assert!(has_stale_in_memory_media_urls(&response));
    }

    #[test]
    fn ignores_non_stale_media_urls() {
        let response = MatrixGetChatMessagesResponse {
            room_id: String::from("!room:example.org"),
            next_from: None,
            messages: vec![
                message_with_image(None),
                message_with_image(Some("asset://localhost/tmp/img-123.png")),
                message_with_image(Some("https://example.org/media.png")),
            ],
        };

        assert!(!has_stale_in_memory_media_urls(&response));
    }
}
