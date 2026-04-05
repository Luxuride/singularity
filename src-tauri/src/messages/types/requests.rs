use serde::Deserialize;

use super::domain::MatrixMessageLoadKind;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatMessagesRequest {
    pub room_id: String,
    pub from: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixStreamChatMessagesRequest {
    pub room_id: String,
    pub from: Option<String>,
    pub limit: Option<u32>,
    pub stream_id: String,
    pub load_kind: MatrixMessageLoadKind,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSendChatMessageRequest {
    pub room_id: String,
    pub body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixToggleReactionRequest {
    pub room_id: String,
    pub target_event_id: String,
    pub key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetUserAvatarRequest {
    pub room_id: String,
    pub user_id: String,
}
