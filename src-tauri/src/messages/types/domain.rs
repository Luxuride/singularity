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
    pub in_reply_to_event_id: Option<String>,
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
