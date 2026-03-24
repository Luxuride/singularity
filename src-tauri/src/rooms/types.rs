use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixRoomKind {
    Room,
    Space,
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChatSummary {
    pub room_id: String,
    pub display_name: String,
    pub encrypted: bool,
    pub joined_members: u64,
    pub kind: MatrixRoomKind,
    pub joined: bool,
    pub is_direct: bool,
    pub parent_room_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatsResponse {
    pub chats: Vec<MatrixChatSummary>,
}
