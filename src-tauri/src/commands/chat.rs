//! Chat command adapters: message fetch/stream, send (text + media), reactions,
//! emoji packs, user avatars, and clipboard. Delegates to the `chat` crate.

use std::sync::Arc;

use tauri::State;

use auth::AuthState;
use storage::AppDb;
use types::chat::{
    MatrixCancelMediaTranscodeRequest, MatrixCancelMediaTranscodeResponse,
    MatrixChatMessageStreamEvent, MatrixCopyImageToClipboardRequest, MatrixDownloadFileRequest,
    MatrixDownloadFileResponse, MatrixGetChatMessagesRequest, MatrixGetChatMessagesResponse,
    MatrixGetEmojiPacksResponse, MatrixGetUserAvatarRequest, MatrixGetUserAvatarResponse,
    MatrixResolveVideoUrlRequest, MatrixResolveVideoUrlResponse, MatrixSendChatMessageRequest,
    MatrixSendChatMessageResponse, MatrixSendMediaFileRequest, MatrixSendMediaFileResponse,
    MatrixStreamChatMessagesRequest, MatrixStreamChatMessagesResponse, MatrixToggleReactionRequest,
    MatrixToggleReactionResponse,
};
use types::{event_paths, Paths, RoomRefreshTrigger, RoomUpdateTriggerState};

#[tauri::command]
pub async fn matrix_get_chat_messages(
    request: MatrixGetChatMessagesRequest,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    paths: State<'_, Paths>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    log::info!("matrix_get_chat_messages requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    let from = request.from.clone();
    let cacheable_initial_request =
        chat::persistence::is_cacheable_initial_request(from.as_deref(), request.limit);
    let limit = request.limit;

    if let Some(cached) = chat::persistence::load_initial_room_messages(
        &app_db,
        request.room_id.as_str(),
        from.as_deref(),
        limit,
    )? {
        let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: Some(request.room_id.clone()),
            include_selected_messages: true,
        });

        if !chat::helpers::has_stale_cached_media_urls(&cached.messages) {
            return Ok(cached);
        }
    }

    let response = match chat::get_chat_messages(
        &client,
        request.room_id.as_str(),
        from.clone(),
        limit,
    )
    .await
    {
        Ok(response) => response,
        Err(error) if chat::helpers::is_room_unavailable_error(&error) => {
            protocol::sync::sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
                .await
                .map_err(|sync_error| {
                    format!(
                        "Failed to sync Matrix room messages after room-unavailable error: {sync_error}"
                    )
                })?;

            chat::get_chat_messages(&client, request.room_id.as_str(), from, limit).await?
        }
        Err(error) => return Err(error),
    };

    let response = MatrixGetChatMessagesResponse {
        room_id: response.room_id,
        next_from: response.next_from,
        messages: response.messages.into_iter().rev().collect(),
    };

    if cacheable_initial_request {
        chat::persistence::store_initial_room_messages(&app_db, &response)?;
    }

    Ok(response)
}

#[tauri::command]
pub async fn matrix_stream_chat_messages(
    request: MatrixStreamChatMessagesRequest,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    event_sink: State<'_, Arc<dyn types::EventSink>>,
    paths: State<'_, Paths>,
) -> Result<MatrixStreamChatMessagesResponse, String> {
    log::info!("matrix_stream_chat_messages requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    let app_db = app_db.inner().clone();
    let trigger_state = room_update_trigger_state.inner().clone();
    let event_sink = event_sink.inner().clone();
    let client_for_task = client.clone();
    let request_for_task = request.clone();
    let terminal_room_id = request.room_id.clone();
    let terminal_stream_id = request.stream_id.clone();
    let terminal_load_kind = request.load_kind;

    tauri::async_runtime::spawn(async move {
        let context = chat::receive::StreamRoomMessagesContext {
            event_sink: event_sink.as_ref(),
            app_db: &app_db,
            room_update_trigger_state: &trigger_state,
            client: &client_for_task,
        };

        let stream_result = chat::stream_chat_messages(context, request_for_task.clone()).await;

        let mut stream_failed = false;

        if let Err(error) = stream_result {
            if chat::helpers::is_room_unavailable_error(&error) {
                if let Err(sync_error) = protocol::sync::sync_once_serialized(
                    &client_for_task,
                    matrix_sdk::config::SyncSettings::default(),
                )
                .await
                {
                    log::warn!(
                        "Background matrix stream sync failed after room-unavailable error: {sync_error}"
                    );
                    stream_failed = true;
                } else {
                    let context = chat::receive::StreamRoomMessagesContext {
                        event_sink: event_sink.as_ref(),
                        app_db: &app_db,
                        room_update_trigger_state: &trigger_state,
                        client: &client_for_task,
                    };
                    if let Err(retry_error) =
                        chat::stream_chat_messages(context, request_for_task).await
                    {
                        log::warn!("Background matrix stream retry failed: {retry_error}");
                        stream_failed = true;
                    }
                }
            } else {
                log::warn!("Background matrix stream failed: {error}");
                stream_failed = true;
            }
        }

        // The frontend only clears its loading state on a terminal `done`
        // event. If the stream failed without emitting one, emit a terminal
        // `done` (with no messages) so the UI never hangs in an infinite
        // loading loop.
        if stream_failed {
            match serde_json::to_value(MatrixChatMessageStreamEvent {
                room_id: terminal_room_id,
                stream_id: terminal_stream_id,
                load_kind: terminal_load_kind,
                sequence: 0,
                message: None,
                next_from: None,
                done: true,
            }) {
                Ok(payload) => {
                    let _ = event_sink.emit(event_paths::CHAT_MESSAGES_STREAM, &payload);
                }
                Err(error) => {
                    log::warn!("Failed to serialize chat message stream completion: {error}");
                }
            }
        }
    });

    Ok(MatrixStreamChatMessagesResponse {
        stream_id: request.stream_id,
        started: true,
    })
}

#[tauri::command]
pub async fn matrix_get_emoji_packs(
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixGetEmojiPacksResponse, String> {
    log::info!("matrix_get_emoji_packs requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::get_emoji_packs(&client).await
}

#[tauri::command]
pub async fn matrix_get_user_avatar(
    request: MatrixGetUserAvatarRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixGetUserAvatarResponse, String> {
    log::info!("matrix_get_user_avatar requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::get_user_avatar(&client, request.room_id.as_str(), request.user_id.as_str()).await
}

#[tauri::command]
pub async fn matrix_send_chat_message(
    request: MatrixSendChatMessageRequest,
    auth_state: State<'_, Arc<AuthState>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSendChatMessageResponse, String> {
    log::info!("matrix_send_chat_message requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    protocol::sync::sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix before send: {error}"))?;

    let room_id = request.room_id.clone();
    let picker_custom_emoji = chat::emoji::load_picker_assets_from_client(&client).await?;
    let response = chat::send_chat_message(
        &client,
        room_id.as_str(),
        request.body.as_str(),
        &picker_custom_emoji,
        request.in_reply_to_event_id.as_deref(),
    )
    .await?;

    let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: Some(room_id),
        include_selected_messages: false,
    });

    Ok(response)
}

#[tauri::command]
pub async fn matrix_send_media_file(
    request: MatrixSendMediaFileRequest,
    auth_state: State<'_, Arc<AuthState>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    media_transcode_cancellation_state: State<'_, chat::MediaTranscodeCancellationState>,
    event_sink: State<'_, Arc<dyn types::EventSink>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSendMediaFileResponse, String> {
    log::info!("matrix_send_media_file requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    protocol::sync::sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix before send: {error}"))?;

    let room_id = request.room_id.clone();
    let response = chat::send_media_file(
        &client,
        &event_sink,
        media_transcode_cancellation_state.inner(),
        room_id.as_str(),
        request.file_path.as_str(),
        request.compress_media,
    )
    .await?;

    let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: Some(room_id),
        include_selected_messages: false,
    });

    Ok(response)
}

#[tauri::command]
pub async fn matrix_cancel_media_transcode(
    request: MatrixCancelMediaTranscodeRequest,
    media_transcode_cancellation_state: State<'_, chat::MediaTranscodeCancellationState>,
) -> Result<MatrixCancelMediaTranscodeResponse, String> {
    let cancelled = chat::cancel_media_transcode(
        media_transcode_cancellation_state.inner(),
        request.room_id.as_str(),
        request.file_path.as_str(),
    )
    .await;

    Ok(MatrixCancelMediaTranscodeResponse { cancelled })
}

#[tauri::command]
pub async fn matrix_toggle_reaction(
    request: MatrixToggleReactionRequest,
    auth_state: State<'_, Arc<AuthState>>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixToggleReactionResponse, String> {
    log::info!("matrix_toggle_reaction requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    protocol::sync::sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix before reaction toggle: {error}"))?;

    let room_id = request.room_id.clone();
    let response = chat::toggle_reaction(
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

    Ok(response)
}

#[tauri::command]
pub async fn matrix_copy_image_to_clipboard(
    request: MatrixCopyImageToClipboardRequest,
) -> Result<(), String> {
    log::info!("matrix_copy_image_to_clipboard requested");
    chat::copy_image_to_clipboard(request.image_url.as_str()).await
}

#[tauri::command]
pub async fn matrix_read_clipboard_text() -> Result<String, String> {
    log::info!("matrix_read_clipboard_text requested");
    chat::read_clipboard_text().await
}

#[tauri::command]
pub async fn matrix_resolve_video_url(
    request: MatrixResolveVideoUrlRequest,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
) -> Result<MatrixResolveVideoUrlResponse, String> {
    log::info!("matrix_resolve_video_url requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;
    chat::resolve_video_url(&client, request.room_id.as_str(), request.event_id.as_str()).await
}

#[tauri::command]
pub async fn matrix_download_file(
    request: MatrixDownloadFileRequest,
    app: tauri::AppHandle,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
) -> Result<MatrixDownloadFileResponse, String> {
    log::info!("matrix_download_file requested");
    let client = auth_state.restore_client_and_get(&paths, &app_db).await?;

    let Some(data) =
        chat::download_file_data(&client, request.room_id.as_str(), request.event_id.as_str())
            .await?
    else {
        return Ok(MatrixDownloadFileResponse { saved: false });
    };

    let default_name = data
        .file_name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            let extension = assets::file_extension_from_mime(
                data.mime_type
                    .as_deref()
                    .unwrap_or("application/octet-stream"),
            );
            format!("download.{extension}")
        });

    use tauri_plugin_dialog::DialogExt;
    let destination = app
        .dialog()
        .file()
        .set_title("Save file")
        .set_file_name(default_name)
        .blocking_save_file();

    let Some(destination) = destination else {
        // User cancelled the save dialog.
        return Ok(MatrixDownloadFileResponse { saved: false });
    };

    let destination = destination
        .into_path()
        .map_err(|error| format!("Failed to resolve save path: {error}"))?;

    assets::save_file_to_path(&data.bytes, &destination)?;

    Ok(MatrixDownloadFileResponse { saved: true })
}
