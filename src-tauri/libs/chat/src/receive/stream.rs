use std::collections::HashSet;
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
use super::fetch::FetchedPage;
use super::parsing::ReactionAccumulator;
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
    /// Reactions seen across every page fetched so far, keyed by the event
    /// they annotate. Handed forward by the fetch layer.
    reactions_by_target: ReactionAccumulator,
    /// Event ids already buffered or emitted. A homeserver that echoes a
    /// non-advancing `from` token back as `end` makes the same page arrive
    /// twice, which would consume the message budget and put the duplicates in
    /// the persisted cache, where the frontend's live-path dedup is not in play.
    seen_event_ids: HashSet<String>,
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
            reactions_by_target: ReactionAccumulator::new(),
            seen_event_ids: HashSet::new(),
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

    fn enqueue_room_snapshot_refresh(&self, room_id: &str) {
        // Sidebar-only: the room list still has to be reconciled, but a cache
        // hit must not trigger a message re-fetch on top of the emission it
        // has just produced.
        let _ = self
            .room_update_trigger_state()
            .enqueue(RoomRefreshTrigger {
                selected_room_id: Some(room_id.to_string()),
                include_selected_messages: false,
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
                // Return false and let this stream re-fetch inline; waking the
                // worker as well would mean two concurrent full fetches of the
                // same page, emitted under two different stream ids.
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

            self.enqueue_room_snapshot_refresh(request.room_id.as_str());
            return Ok(true);
        }

        Ok(false)
    }

    /// Fetch batches from the server, emitting non-initial messages inline and
    /// buffering initial messages for reverse emission.
    ///
    /// The fetcher is passed in rather than called directly so this stays
    /// testable without a homeserver, and so the reaction accumulator lives in
    /// one place across every page.
    async fn fetch_and_emit_batches<F, Fut>(
        &mut self,
        request: &MatrixStreamChatMessagesRequest,
        mut fetch_page: F,
    ) -> Result<(), String>
    where
        F: FnMut(String, Option<String>, Option<u32>) -> Fut,
        Fut: std::future::Future<Output = Result<FetchedPage, String>>,
    {
        while self.sequence < self.target_message_count as u32
            && self.request_count < self.max_request_count
        {
            let remaining = self
                .target_message_count
                .saturating_sub(self.sequence as usize);
            let batch_limit = remaining.min(50) as u32;
            let previous_scan_from = self.scan_from.clone();

            let page = fetch_page(
                request.room_id.clone(),
                self.scan_from.clone(),
                Some(batch_limit),
            )
            .await?;

            self.request_count = self.request_count.saturating_add(1);
            let raw_event_count = page.raw_event_count;
            // Carry the accumulator forward: a reaction parsed on this page
            // usually annotates a message on a LATER (older) page.
            self.reactions_by_target = page.reactions_by_target;
            let MatrixGetChatMessagesResponse {
                next_from,
                messages,
                ..
            } = page.response;
            self.final_next_from = next_from.clone();
            self.scan_from = next_from;

            // A repeated cursor means the server is not making progress, not
            // that we should keep fetching.
            let cursor_repeated = self.scan_from == previous_scan_from;
            if cursor_repeated {
                self.same_cursor_count = self.same_cursor_count.saturating_add(1);
            } else {
                self.same_cursor_count = 0;
            }

            for message in messages {
                if self.sequence >= self.target_message_count as u32 {
                    break;
                }

                if let Some(event_id) = &message.event_id {
                    if !self.seen_event_ids.insert(event_id.clone()) {
                        continue;
                    }
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

            // Stop on an empty *raw* page. A page of nothing but reactions,
            // state events or undecryptable events parses to zero messages
            // while the server is still holding more history.
            if raw_event_count == 0 {
                break;
            }

            if cursor_repeated {
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

        let initial_messages = std::mem::take(&mut self.initial_messages);

        for message in initial_messages.into_iter().rev() {
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
        mut fetch_page: F,
    ) -> Result<MatrixStreamChatMessagesResponse, String>
    where
        F: FnMut(String, Option<String>, Option<u32>) -> Fut,
        Fut: std::future::Future<Output = Result<FetchedPage, String>>,
    {
        if self.try_serve_from_cache(&request)? {
            return Ok(MatrixStreamChatMessagesResponse {
                stream_id: request.stream_id.clone(),
                started: true,
            });
        }

        let fetch_result = self.fetch_and_emit_batches(&request, &mut fetch_page).await;

        if let Err(error) = fetch_result {
            // An initial load buffers everything and only flushes once the
            // whole loop returns, so the partial progress is flushed,
            // persisted and published with the cursor it did advance to before
            // the failure is reported.
            if matches!(request.load_kind, MatrixMessageLoadKind::Initial)
                && !self.initial_messages.is_empty()
            {
                self.emit_initial_batches(&request)?;
                self.store_cache(&request)?;
                self.emit_completion(
                    request.room_id.as_str(),
                    request.stream_id.as_str(),
                    request.load_kind,
                    self.final_next_from.clone(),
                )?;
            }

            return Err(error);
        }

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
