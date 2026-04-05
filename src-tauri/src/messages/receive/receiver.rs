use tauri::AppHandle;

use crate::db::AppDb;
use crate::rooms::RoomUpdateTriggerState;

use super::super::media::{DefaultMediaResolver, MediaResolver};
use super::super::types::{
    MatrixGetChatMessagesResponse, MatrixStreamChatMessagesRequest,
    MatrixStreamChatMessagesResponse,
};
use super::fetch::fetch_room_messages_impl;
use super::stream::stream_room_messages_impl;

#[derive(Clone, Copy)]
pub(crate) struct StreamRoomMessagesContext<'a> {
    pub(crate) app_handle: &'a AppHandle,
    pub(crate) app_db: &'a AppDb,
    pub(crate) room_update_trigger_state: &'a RoomUpdateTriggerState,
    pub(crate) client: &'a matrix_sdk::Client,
}

pub(crate) trait MessageReceiver {
    async fn fetch_room_messages(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        from: Option<String>,
        limit: Option<u32>,
    ) -> Result<MatrixGetChatMessagesResponse, String>;

    async fn stream_room_messages(
        &self,
        context: StreamRoomMessagesContext<'_>,
        request: MatrixStreamChatMessagesRequest,
    ) -> Result<MatrixStreamChatMessagesResponse, String>;
}

pub(crate) struct MatrixMessageReceiver<M: MediaResolver = DefaultMediaResolver> {
    media_resolver: M,
}

impl Default for MatrixMessageReceiver<DefaultMediaResolver> {
    fn default() -> Self {
        Self {
            media_resolver: DefaultMediaResolver,
        }
    }
}

impl<M: MediaResolver> MessageReceiver for MatrixMessageReceiver<M> {
    async fn fetch_room_messages(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        from: Option<String>,
        limit: Option<u32>,
    ) -> Result<MatrixGetChatMessagesResponse, String> {
        fetch_room_messages_impl(&self.media_resolver, client, room_id_raw, from, limit).await
    }

    async fn stream_room_messages(
        &self,
        context: StreamRoomMessagesContext<'_>,
        request: MatrixStreamChatMessagesRequest,
    ) -> Result<MatrixStreamChatMessagesResponse, String> {
        let client = context.client;
        let media_resolver = &self.media_resolver;
        stream_room_messages_impl(context, request, |room_id_raw, from, limit| async move {
            fetch_room_messages_impl(media_resolver, client, room_id_raw.as_str(), from, limit)
                .await
        })
        .await
    }
}

pub(crate) async fn fetch_room_messages_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    from: Option<String>,
    limit: Option<u32>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    MatrixMessageReceiver::default()
        .fetch_room_messages(client, room_id_raw, from, limit)
        .await
}

pub(crate) async fn stream_room_messages_from_client(
    context: StreamRoomMessagesContext<'_>,
    request: MatrixStreamChatMessagesRequest,
) -> Result<MatrixStreamChatMessagesResponse, String> {
    MatrixMessageReceiver::default()
        .stream_room_messages(context, request)
        .await
}
