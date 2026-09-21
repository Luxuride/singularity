use std::sync::Arc;

use storage::AppDb;
use types::chat::{
    MatrixChatMessage, MatrixChatMessageStreamEvent, MatrixGetChatMessagesResponse,
    MatrixMessageLoadKind, MatrixStreamChatMessagesRequest, MatrixStreamChatMessagesResponse,
};
use types::event_paths;
use types::{EventSink, RoomRefreshTrigger, RoomUpdateTriggerState};

use super::super::helpers::has_stale_cached_media_urls;
use super::super::persistence::{
    is_cacheable_initial_request, load_initial_room_messages, store_initial_room_messages,
};
use super::receiver::StreamRoomMessagesContext;

fn emit_stream_event(
    event_sink: &dyn EventSink,
    event: &MatrixChatMessageStreamEvent,
    label: &str,
) -> Result<(), String> {
    let payload = serde_json::to_value(event)
        .map_err(|error| format!("Failed to serialize chat message stream event: {error}"))?;
    event_sink
        .emit(event_paths::CHAT_MESSAGES_STREAM, &payload)
        .map_err(|error| format!("Failed to emit {label}: {error}"))
}

/// Streams a room's chat messages to the frontend, emitting one event per
/// message followed by a terminal `done` event. Owns the per-stream mutable
/// state (pagination cursor, buffered batches, sequence counter) so each phase
/// of the stream is a small, independently testable method.
pub(super) struct ChatMessageStreamer<'a> {
    context: StreamRoomMessagesContext<'a>,
    target_message_count: usize,
    cacheable_initial_request: bool,
    sequence: u32,
    scan_from: Option<String>,
    cache_messages: Vec<MatrixChatMessage>,
    initial_messages: Vec<MatrixChatMessage>,
    final_next_from: Option<String>,
    request_count: usize,
    same_cursor_count: usize,
    max_request_count: usize,
}

impl<'a> ChatMessageStreamer<'a> {
    pub(super) fn new(
        context: StreamRoomMessagesContext<'a>,
        request: &MatrixStreamChatMessagesRequest,
    ) -> Self {
        let target_message_count = request.limit.unwrap_or(50).clamp(1, 100) as usize;
        let cacheable_initial_request = matches!(request.load_kind, MatrixMessageLoadKind::Initial)
            && is_cacheable_initial_request(request.from.as_deref(), request.limit);

        Self {
            context,
            target_message_count,
            cacheable_initial_request,
            sequence: 0,
            scan_from: request.from.clone(),
            cache_messages: Vec::with_capacity(target_message_count),
            initial_messages: Vec::with_capacity(target_message_count),
            final_next_from: None,
            request_count: 0,
            same_cursor_count: 0,
            max_request_count: ((target_message_count.saturating_add(49)) / 50)
                .saturating_mul(6)
                .max(8),
        }
    }

    fn event_sink(&self) -> &dyn EventSink {
        self.context.event_sink
    }

    fn app_db(&self) -> &Arc<AppDb> {
        self.context.app_db
    }

    fn room_update_trigger_state(&self) -> &RoomUpdateTriggerState {
        self.context.room_update_trigger_state
    }

    fn emit_message(
        &mut self,
        room_id: &str,
        stream_id: &str,
        load_kind: MatrixMessageLoadKind,
        message: MatrixChatMessage,
    ) -> Result<(), String> {
        emit_stream_event(
            self.event_sink(),
            &MatrixChatMessageStreamEvent {
                room_id: room_id.to_string(),
                stream_id: stream_id.to_string(),
                load_kind,
                sequence: self.sequence,
                message: Some(message),
                next_from: None,
                done: false,
            },
            "chat message stream event",
        )?;

        self.sequence = self.sequence.saturating_add(1);
        Ok(())
    }

    fn emit_completion(
        &self,
        room_id: &str,
        stream_id: &str,
        load_kind: MatrixMessageLoadKind,
        next_from: Option<String>,
    ) -> Result<(), String> {
        emit_stream_event(
            self.event_sink(),
            &MatrixChatMessageStreamEvent {
                room_id: room_id.to_string(),
                stream_id: stream_id.to_string(),
                load_kind,
                sequence: self.sequence,
                message: None,
                next_from,
                done: true,
            },
            "chat message stream completion",
        )?;

        Ok(())
    }

    fn enqueue_room_refresh(&self, room_id: &str) {
        let _ = self
            .room_update_trigger_state()
            .enqueue(RoomRefreshTrigger {
                selected_room_id: Some(room_id.to_string()),
                include_selected_messages: true,
            });
    }

    /// Serve a cacheable initial request from the persisted cache when
    /// available. Returns `true` when the stream was fully served from cache.
    fn try_serve_from_cache(
        &mut self,
        request: &MatrixStreamChatMessagesRequest,
    ) -> Result<bool, String> {
        if !self.cacheable_initial_request {
            return Ok(false);
        }

        let cached = load_initial_room_messages(
            self.app_db(),
            request.room_id.as_str(),
            request.from.as_deref(),
            request.limit,
        )?;

        if let Some(cached) = cached {
            if has_stale_cached_media_urls(&cached.messages) {
                self.enqueue_room_refresh(request.room_id.as_str());
                return Ok(false);
            }

            for message in cached.messages {
                self.emit_message(
                    request.room_id.as_str(),
                    request.stream_id.as_str(),
                    request.load_kind,
                    message,
                )?;
            }

            self.emit_completion(
                request.room_id.as_str(),
                request.stream_id.as_str(),
                request.load_kind,
                cached.next_from,
            )?;

            self.enqueue_room_refresh(request.room_id.as_str());
            return Ok(true);
        }

        Ok(false)
    }

    /// Fetch batches from the server, emitting non-initial messages inline and
    /// buffering initial messages for reverse emission.
    async fn fetch_and_emit_batches<F, Fut>(
        &mut self,
        request: &MatrixStreamChatMessagesRequest,
        mut fetch_room_messages: F,
    ) -> Result<(), String>
    where
        F: FnMut(String, Option<String>, Option<u32>) -> Fut,
        Fut: std::future::Future<Output = Result<MatrixGetChatMessagesResponse, String>>,
    {
        while self.sequence < self.target_message_count as u32
            && self.request_count < self.max_request_count
        {
            let remaining = self
                .target_message_count
                .saturating_sub(self.sequence as usize);
            let batch_limit = remaining.min(50) as u32;
            let previous_scan_from = self.scan_from.clone();

            let response = fetch_room_messages(
                request.room_id.clone(),
                self.scan_from.clone(),
                Some(batch_limit),
            )
            .await?;

            self.request_count = self.request_count.saturating_add(1);
            self.final_next_from = response.next_from.clone();
            let message_count = response.messages.len();
            self.scan_from = response.next_from;

            if self.scan_from == previous_scan_from {
                self.same_cursor_count = self.same_cursor_count.saturating_add(1);
            } else {
                self.same_cursor_count = 0;
            }

            for message in response.messages {
                if self.sequence >= self.target_message_count as u32 {
                    break;
                }

                if matches!(request.load_kind, MatrixMessageLoadKind::Initial) {
                    // Matrix backward pagination yields newest->older. Buffer
                    // initial batches and emit once in reverse so the timeline
                    // receives consistent oldest->newest order.
                    self.initial_messages.push(message);
                    self.sequence = self.sequence.saturating_add(1);
                } else {
                    self.emit_message(
                        request.room_id.as_str(),
                        request.stream_id.as_str(),
                        request.load_kind,
                        message,
                    )?;
                }
            }

            if self.scan_from.is_none() {
                break;
            }

            if message_count == 0 {
                break;
            }

            if self.same_cursor_count >= 2 {
                break;
            }
        }

        Ok(())
    }

    /// Emit buffered initial messages in reverse (oldest->newest) order,
    /// collecting cacheable messages along the way.
    fn emit_initial_batches(
        &mut self,
        request: &MatrixStreamChatMessagesRequest,
    ) -> Result<(), String> {
        self.sequence = 0;

        for message in self.initial_messages.clone().into_iter().rev() {
            if self.cacheable_initial_request {
                self.cache_messages.push(message.clone());
            }

            self.emit_message(
                request.room_id.as_str(),
                request.stream_id.as_str(),
                request.load_kind,
                message,
            )?;
        }

        Ok(())
    }

    /// Persist a cacheable initial request's messages for future fast loads.
    fn store_cache(&mut self, request: &MatrixStreamChatMessagesRequest) -> Result<(), String> {
        if !self.cacheable_initial_request {
            return Ok(());
        }

        let cache_messages = self.cache_messages.clone();
        store_initial_room_messages(
            self.app_db(),
            &MatrixGetChatMessagesResponse {
                room_id: request.room_id.clone(),
                next_from: self.final_next_from.clone(),
                messages: cache_messages,
            },
        )?;

        Ok(())
    }

    pub(super) async fn run<F, Fut>(
        &mut self,
        request: MatrixStreamChatMessagesRequest,
        fetch_room_messages: F,
    ) -> Result<MatrixStreamChatMessagesResponse, String>
    where
        F: FnMut(String, Option<String>, Option<u32>) -> Fut,
        Fut: std::future::Future<Output = Result<MatrixGetChatMessagesResponse, String>>,
    {
        if self.try_serve_from_cache(&request)? {
            return Ok(MatrixStreamChatMessagesResponse {
                stream_id: request.stream_id.clone(),
                started: true,
            });
        }

        self.fetch_and_emit_batches(&request, fetch_room_messages)
            .await?;

        if matches!(request.load_kind, MatrixMessageLoadKind::Initial) {
            self.emit_initial_batches(&request)?;
        }

        self.store_cache(&request)?;

        self.emit_completion(
            request.room_id.as_str(),
            request.stream_id.as_str(),
            request.load_kind,
            self.final_next_from.clone(),
        )?;

        Ok(MatrixStreamChatMessagesResponse {
            stream_id: request.stream_id.clone(),
            started: true,
        })
    }
}
