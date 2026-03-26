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
    pub image_url: Option<String>,
    pub encrypted: bool,
    pub joined_members: u64,
    pub kind: MatrixRoomKind,
    pub joined: bool,
    pub is_direct: bool,
    pub parent_room_id: Option<String>,
    #[serde(default)]
    pub parent_room_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatsResponse {
    pub chats: Vec<MatrixChatSummary>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatNavigationRequest {
    pub root_space_id: Option<String>,
    pub selected_room_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatNavigationResponse {
    pub selected_root_space_id: Option<String>,
    pub root_spaces: Vec<MatrixChatSummary>,
    pub root_scoped_rooms: Vec<MatrixChatSummary>,
}
