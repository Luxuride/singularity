use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatMessagesRequest {
    pub room_id: String,
    pub from: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChatMessage {
    pub event_id: Option<String>,
    pub sender: String,
    pub timestamp: Option<u64>,
    pub body: String,
    pub encrypted: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatMessagesResponse {
    pub room_id: String,
    pub next_from: Option<String>,
    pub messages: Vec<MatrixChatMessage>,
}
