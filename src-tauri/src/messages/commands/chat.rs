use log::info;
use tauri::{AppHandle, Manager, State};

use crate::auth::AuthState;
use crate::db::AppDb;
use crate::protocol::sync::sync_once_serialized;
use crate::rooms::{RoomRefreshTrigger, RoomUpdateTriggerState};

use super::super::helpers::{has_stale_in_memory_media_urls, is_room_unavailable_error};
use super::super::persistence::{
    is_cacheable_initial_request, load_initial_room_messages, store_initial_room_messages,
};
use super::super::receive::{
    fetch_room_messages_from_client, stream_room_messages_from_client, StreamRoomMessagesContext,
};
use super::super::types::{
    MatrixGetChatMessagesRequest, MatrixGetChatMessagesResponse, MatrixStreamChatMessagesRequest,
    MatrixStreamChatMessagesResponse,
};

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

        if !has_stale_in_memory_media_urls(&cached.messages) {
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

    let response = MatrixGetChatMessagesResponse {
        room_id: response.room_id,
        next_from: response.next_from,
        messages: response.messages.into_iter().rev().collect(),
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
    _app_db: State<'_, AppDb>,
    _room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixStreamChatMessagesResponse, String> {
    info!("matrix_stream_chat_messages requested");
    let client = auth_state.restore_client_and_get(&app_handle).await?;
    let app_handle_for_task = app_handle.clone();
    let client_for_task = client.clone();
    let request_for_task = request.clone();

    tauri::async_runtime::spawn(async move {
        let app_db = app_handle_for_task.state::<AppDb>();
        let room_update_trigger_state = app_handle_for_task.state::<RoomUpdateTriggerState>();

        let stream_result = stream_room_messages_from_client(
            StreamRoomMessagesContext {
                app_handle: &app_handle_for_task,
                app_db: &app_db,
                room_update_trigger_state: &room_update_trigger_state,
                client: &client_for_task,
            },
            request_for_task.clone(),
        )
        .await;

        if let Err(error) = stream_result {
            if is_room_unavailable_error(&error) {
                if let Err(sync_error) = sync_once_serialized(
                    &client_for_task,
                    matrix_sdk::config::SyncSettings::default(),
                )
                .await
                {
                    log::warn!(
                        "Background matrix stream sync failed after room-unavailable error: {sync_error}"
                    );
                } else if let Err(retry_error) = stream_room_messages_from_client(
                    StreamRoomMessagesContext {
                        app_handle: &app_handle_for_task,
                        app_db: &app_db,
                        room_update_trigger_state: &room_update_trigger_state,
                        client: &client_for_task,
                    },
                    request_for_task,
                )
                .await
                {
                    log::warn!("Background matrix stream retry failed: {retry_error}");
                }
            } else {
                log::warn!("Background matrix stream failed: {error}");
            }
        }
    });

    Ok(MatrixStreamChatMessagesResponse {
        stream_id: request.stream_id,
        started: true,
    })
}
