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
    #[serde(default)]
    pub join_rule: Option<String>,
    #[serde(default)]
    pub world_readable: Option<bool>,
    #[serde(default)]
    pub guest_can_join: Option<bool>,
    #[serde(default)]
    pub children_room_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatsResponse {
    pub chats: Vec<MatrixChatSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetRoomImageRequest {
    pub room_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetRoomImageResponse {
    pub room_id: String,
    pub image_url: Option<String>,
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

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetRootSpaceOrderRequest {
    #[serde(default)]
    pub root_space_ids: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetRootSpaceOrderResponse {
    pub root_space_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixJoinRoomRequest {
    pub room_id_or_alias: String,
    pub server_names: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixJoinRoomResponse {
    pub room_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetSpaceBrowseRequest {
    pub root_space_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetSpaceBrowseResponse {
    pub root_space_id: String,
    pub rooms: Vec<MatrixChatSummary>,
}
