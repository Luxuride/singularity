use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixMessageLoadKind {
    Initial,
    Older,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixMessageDecryptionStatus {
    Plaintext,
    Decrypted,
    UnableToDecrypt,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixMessageVerificationStatus {
    Unknown,
    Verified,
    Unverified,
}

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

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCustomEmoji {
    pub shortcode: String,
    pub url: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixPickerCustomEmoji {
    pub name: String,
    pub shortcodes: Vec<String>,
    pub url: String,
    pub source_url: String,
    pub category: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixReactionSummary {
    pub key: String,
    pub count: u32,
    pub senders: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChatMessage {
    pub event_id: Option<String>,
    pub sender: String,
    pub timestamp: Option<u64>,
    pub body: String,
    pub formatted_body: Option<String>,
    pub message_type: Option<String>,
    pub image_url: Option<String>,
    #[serde(default)]
    pub custom_emojis: Vec<MatrixCustomEmoji>,
    #[serde(default)]
    pub reactions: Vec<MatrixReactionSummary>,
    pub encrypted: bool,
    pub decryption_status: MatrixMessageDecryptionStatus,
    pub verification_status: MatrixMessageVerificationStatus,
}

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
pub struct MatrixGetEmojiPacksResponse {
    pub custom_emoji: Vec<MatrixPickerCustomEmoji>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChatMessageStreamEvent {
    pub room_id: String,
    pub stream_id: String,
    pub load_kind: MatrixMessageLoadKind,
    pub sequence: u32,
    pub message: Option<MatrixChatMessage>,
    pub next_from: Option<String>,
    pub done: bool,
}
