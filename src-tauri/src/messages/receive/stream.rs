use tauri::Emitter;

use crate::protocol::event_paths;
use crate::rooms::RoomRefreshTrigger;

use super::super::helpers::has_stale_in_memory_media_urls;
use super::super::media::DefaultMediaResolver;
use super::super::persistence::{
    is_cacheable_initial_request, load_initial_room_messages, store_initial_room_messages,
};
use super::super::types::{
    MatrixChatMessageImageLoadedEvent, MatrixChatMessageStreamEvent, MatrixGetChatMessagesResponse,
    MatrixMessageLoadKind, MatrixStreamChatMessagesRequest, MatrixStreamChatMessagesResponse,
};
use super::super::media::MediaResolver;
use super::receiver::StreamRoomMessagesContext;

pub(super) async fn stream_room_messages_impl<F, Fut>(
    context: StreamRoomMessagesContext<'_>,
    request: MatrixStreamChatMessagesRequest,
    mut fetch_room_messages: F,
) -> Result<MatrixStreamChatMessagesResponse, String>
where
    F: FnMut(String, Option<String>, Option<u32>) -> Fut,
    Fut: std::future::Future<Output = Result<MatrixGetChatMessagesResponse, String>>,
{
    let StreamRoomMessagesContext {
        app_handle,
        app_db,
        room_update_trigger_state,
        ..
    } = context;
    let MatrixStreamChatMessagesRequest {
        room_id,
        from,
        limit,
        stream_id,
        load_kind,
    } = request;

    let target_message_count = limit.unwrap_or(50).clamp(1, 100) as usize;
    let cacheable_initial_request = matches!(load_kind, MatrixMessageLoadKind::Initial)
        && is_cacheable_initial_request(from.as_deref(), limit);

    if cacheable_initial_request {
        if let Some(cached) =
            load_initial_room_messages(app_db, room_id.as_str(), from.as_deref(), limit)?
        {
            if has_stale_in_memory_media_urls(&cached.messages) {
                let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
                    selected_room_id: Some(room_id.clone()),
                    include_selected_messages: true,
                });
            } else {
                let cached_messages = cached.messages;
                let mut sequence = 0_u32;

                for message in cached_messages {
                    spawn_image_resolution_for_stream(
                        &context,
                        room_id.clone(),
                        message.event_id.clone(),
                        message.image_url.clone(),
                    );

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
                    include_selected_messages: true,
                });

                return Ok(MatrixStreamChatMessagesResponse {
                    stream_id,
                    started: true,
                });
            }
        }
    }

    let mut scan_from = from;
    let mut cache_messages = Vec::with_capacity(target_message_count);
    let mut initial_messages = Vec::with_capacity(target_message_count);
    let mut final_next_from = None;
    let mut request_count = 0_usize;
    let mut same_cursor_count = 0_usize;
    let max_request_count = ((target_message_count.saturating_add(49)) / 50)
        .saturating_mul(6)
        .max(8);
    let mut sequence = 0_u32;

    while sequence < target_message_count as u32 && request_count < max_request_count {
        let remaining = target_message_count.saturating_sub(sequence as usize);
        let batch_limit = remaining.min(50) as u32;
        let previous_scan_from = scan_from.clone();

        let response =
            fetch_room_messages(room_id.clone(), scan_from.clone(), Some(batch_limit)).await?;

        request_count = request_count.saturating_add(1);
        final_next_from = response.next_from.clone();
        let message_count = response.messages.len();
        scan_from = response.next_from;

        if scan_from == previous_scan_from {
            same_cursor_count = same_cursor_count.saturating_add(1);
        } else {
            same_cursor_count = 0;
        }

        for message in response.messages {
            if sequence >= target_message_count as u32 {
                break;
            }

            if matches!(load_kind, MatrixMessageLoadKind::Initial) {
                // Matrix backward pagination yields newest->older. Buffer initial
                // batches and emit once in reverse so the timeline receives
                // consistent oldest->newest order.
                initial_messages.push(message);
            } else {
                spawn_image_resolution_for_stream(
                    &context,
                    room_id.clone(),
                    message.event_id.clone(),
                    message.image_url.clone(),
                );

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
            }

            sequence = sequence.saturating_add(1);
        }

        if scan_from.is_none() {
            break;
        }

        if message_count == 0 {
            break;
        }

        if same_cursor_count >= 2 {
            break;
        }
    }

    if matches!(load_kind, MatrixMessageLoadKind::Initial) {
        sequence = 0;

        for message in initial_messages.into_iter().rev() {
            if cacheable_initial_request {
                cache_messages.push(message.clone());
            }

            spawn_image_resolution_for_stream(
                &context,
                room_id.clone(),
                message.event_id.clone(),
                message.image_url.clone(),
            );

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
    }

    let next_from = final_next_from.clone();

    if cacheable_initial_request {
        store_initial_room_messages(
            app_db,
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

fn spawn_image_resolution_for_stream(
    context: &StreamRoomMessagesContext<'_>,
    room_id: String,
    message_event_id: Option<String>,
    image_url: Option<String>,
) {
    let Some(event_id) = message_event_id else {
        return;
    };

    let Some(raw_image_url) = image_url else {
        return;
    };

    if raw_image_url.trim().is_empty()
        || raw_image_url.starts_with("asset://")
        || raw_image_url.starts_with("matrix-media://")
        || raw_image_url.starts_with("file://")
        || raw_image_url.starts_with('/')
    {
        return;
    }

    let client = context.client.clone();
    let app_handle = context.app_handle.clone();

    tauri::async_runtime::spawn(async move {
        let resolver = DefaultMediaResolver;
        let Some(resolved_image_url) = resolver.resolve_pack_media_url(&client, &raw_image_url).await else {
            return;
        };

        if resolved_image_url == raw_image_url {
            return;
        }

        let _ = app_handle.emit(
            event_paths::CHAT_MESSAGE_IMAGE_LOADED,
            MatrixChatMessageImageLoadedEvent {
                room_id,
                event_id,
                image_url: resolved_image_url,
            },
        );
    });
}
