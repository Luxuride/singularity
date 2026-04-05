use serde::{Deserialize, Serialize};

use super::domain::{MatrixChatMessage, MatrixPickerCustomEmoji};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatMessagesResponse {
    pub room_id: String,
    pub next_from: Option<String>,
    pub messages: Vec<MatrixChatMessage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixStreamChatMessagesResponse {
    pub stream_id: String,
    pub started: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSendChatMessageResponse {
    pub event_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixToggleReactionResponse {
    pub added: bool,
    pub event_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetUserAvatarResponse {
    pub user_id: String,
    pub image_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetEmojiPacksResponse {
    pub custom_emoji: Vec<MatrixPickerCustomEmoji>,
}
