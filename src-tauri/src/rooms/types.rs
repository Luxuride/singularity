use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChatSummary {
    pub room_id: String,
    pub display_name: String,
    pub encrypted: bool,
    pub joined_members: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatsResponse {
    pub chats: Vec<MatrixChatSummary>,
}
