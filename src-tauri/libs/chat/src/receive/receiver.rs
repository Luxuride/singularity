use std::sync::Arc;

use storage::AppDb;
use types::{EventSink, RoomUpdateTriggerState};

use super::super::media::DefaultMediaResolver;
use super::fetch::fetch_room_messages_impl;
use super::parsing::ReactionAccumulator;
use super::stream::ChatMessageStreamer;
use types::chat::{
    MatrixGetChatMessagesResponse, MatrixStreamChatMessagesRequest,
    MatrixStreamChatMessagesResponse,
};

#[derive(Clone, Copy)]
pub struct StreamRoomMessagesContext<'a> {
    pub event_sink: &'a dyn EventSink,
    pub app_db: &'a Arc<AppDb>,
    pub room_update_trigger_state: &'a RoomUpdateTriggerState,
    pub client: &'a matrix_sdk::Client,
}

pub async fn fetch_room_messages_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    from: Option<String>,
    limit: Option<u32>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    // A one-shot fetch is a single page, so reactions from a previous page are
    // simply not in scope for a fresh load; the ones inside this page still
    // land on their targets.
    let mut reactions = ReactionAccumulator::new();
    fetch_room_messages_impl(
        &DefaultMediaResolver,
        client,
        room_id_raw,
        from,
        limit,
        &mut reactions,
    )
    .await
    .map(|page| page.response)
}

pub async fn stream_room_messages_from_client(
    context: StreamRoomMessagesContext<'_>,
    request: MatrixStreamChatMessagesRequest,
) -> Result<MatrixStreamChatMessagesResponse, String> {
    let client = context.client;
    let media_resolver = &DefaultMediaResolver;
    let mut streamer = ChatMessageStreamer::new(context, &request);
    streamer
        .run(request, |room_id_raw, from, limit| async move {
            fetch_room_messages_impl(
                media_resolver,
                client,
                room_id_raw.as_str(),
                from,
                limit,
                &mut ReactionAccumulator::new(),
            )
            .await
        })
        .await
}
