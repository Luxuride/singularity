use log::info;
use tauri::{AppHandle, Emitter, State};

use crate::auth::AuthState;
use crate::db::AppDb;
use crate::protocol::event_paths;
use crate::protocol::sync::sync_once_serialized;
use crate::rooms::{RoomRefreshTrigger, RoomUpdateTriggerState};

use super::persistence::{is_cacheable_initial_request, load_initial_room_messages, store_initial_room_messages};
use super::types::{
    MatrixChatMessageStreamEvent, MatrixGetChatMessagesRequest, MatrixGetChatMessagesResponse,
    MatrixMessageLoadKind, MatrixSendChatMessageRequest, MatrixSendChatMessageResponse,
    MatrixStreamChatMessagesRequest, MatrixStreamChatMessagesResponse,
};
use super::workers::{fetch_room_messages_from_client, send_room_message_from_client};

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
        });
        return Ok(cached);
    }

    sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix room messages: {error}"))?;

    let response =
        fetch_room_messages_from_client(&client, request.room_id.as_str(), from, limit).await?;

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

    let room_id = request.room_id;
    let stream_id = request.stream_id;
    let load_kind = request.load_kind;
    let limit = request.limit;
    let from = request.from;
    let target_message_count = limit.unwrap_or(50).clamp(1, 100) as usize;
    let cacheable_initial_request = matches!(load_kind, MatrixMessageLoadKind::Initial)
        && is_cacheable_initial_request(from.as_deref(), limit);

    if cacheable_initial_request {
        if let Some(cached) =
            load_initial_room_messages(&app_db, room_id.as_str(), from.as_deref(), limit)?
        {
            let iter = cached.messages.into_iter();
            let mut sequence = 0_u32;

            for message in iter {
                app_handle
                    .emit(
                        event_paths::CHAT_MESSAGES_STREAM,
                        MatrixChatMessageStreamEvent {
                            room_id: room_id.clone(),
                            stream_id: stream_id.clone(),
                            load_kind,
                            sequence,
                            message: Some(message),
                            next_from: None,
                            done: false,
                        },
                    )
                    .map_err(|error| {
                        format!("Failed to emit chat message stream event: {error}")
                    })?;

                sequence = sequence.saturating_add(1);
            }

            app_handle
                .emit(
                    event_paths::CHAT_MESSAGES_STREAM,
                    MatrixChatMessageStreamEvent {
                        room_id: room_id.clone(),
                        stream_id: stream_id.clone(),
                        load_kind,
                        sequence,
                        message: None,
                        next_from: cached.next_from,
                        done: true,
                    },
                )
                .map_err(|error| {
                    format!("Failed to emit chat message stream completion: {error}")
                })?;

            let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
                selected_room_id: Some(room_id),
            });

            return Ok(MatrixStreamChatMessagesResponse {
                stream_id,
                started: true,
            });
        }
    }

    sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix room messages: {error}"))?;

    let mut scan_from = from;
    let mut cache_messages = Vec::with_capacity(target_message_count);
    let mut final_next_from = None;
    let mut request_count = 0_usize;
    let max_request_count = target_message_count.saturating_mul(20);
    let mut sequence = 0_u32;

    while sequence < target_message_count as u32 && request_count < max_request_count {
        let remaining = target_message_count.saturating_sub(sequence as usize);
        let batch_limit = remaining.min(10) as u32;

        let response = fetch_room_messages_from_client(
            &client,
            room_id.as_str(),
            scan_from.clone(),
            Some(batch_limit),
        )
        .await?;

        request_count = request_count.saturating_add(1);
        final_next_from = response.next_from.clone();
        scan_from = response.next_from;

        for message in response.messages {
            if sequence >= target_message_count as u32 {
                break;
            }

            if cacheable_initial_request {
                cache_messages.push(message.clone());
            }

            app_handle
                .emit(
                    event_paths::CHAT_MESSAGES_STREAM,
                    MatrixChatMessageStreamEvent {
                        room_id: room_id.clone(),
                        stream_id: stream_id.clone(),
                        load_kind,
                        sequence,
                        message: Some(message),
                        next_from: None,
                        done: false,
                    },
                )
                .map_err(|error| format!("Failed to emit chat message stream event: {error}"))?;

            sequence = sequence.saturating_add(1);
        }

        if scan_from.is_none() {
            break;
        }
    }

    let next_from = final_next_from.clone();

    if cacheable_initial_request {
        store_initial_room_messages(
            &app_db,
            &MatrixGetChatMessagesResponse {
                room_id: room_id.clone(),
                next_from: next_from.clone(),
                messages: cache_messages,
            },
        )?;
    }

    app_handle
        .emit(
            event_paths::CHAT_MESSAGES_STREAM,
            MatrixChatMessageStreamEvent {
                room_id,
                stream_id: stream_id.clone(),
                load_kind,
                sequence,
                message: None,
                next_from,
                done: true,
            },
        )
        .map_err(|error| format!("Failed to emit chat message stream completion: {error}"))?;

    Ok(MatrixStreamChatMessagesResponse {
        stream_id,
        started: true,
    })
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
    let event_id =
        send_room_message_from_client(&client, room_id.as_str(), request.body.as_str()).await?;

    let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: Some(room_id),
    });

    Ok(MatrixSendChatMessageResponse { event_id })
}
