use serde::{Deserialize, Serialize};

/// The signed-in user's profile fields.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixProfileResponse {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetDisplayNameRequest {
    pub display_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetDisplayNameResponse {
    pub set: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetAvatarRequest {
    /// Absolute path of the image file to upload as the new avatar.
    pub file_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetAvatarResponse {
    pub mxc_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRemoveAvatarRequest {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRemoveAvatarResponse {
    pub removed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChangePasswordRequest {
    pub new_password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChangePasswordResponse {
    pub changed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetRoomSettingsRequest {
    pub room_id: String,
}

/// The user's own membership-level settings for one room or space.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRoomSettingsResponse {
    pub room_id: String,
    pub kind: crate::rooms::MatrixRoomKind,
    /// The effective notification mode: the user-defined one when set, the
    /// room-type default otherwise. `None` when the room is not joined.
    pub notification_mode: Option<MatrixRoomNotificationMode>,
    /// Whether a user-defined notification rule exists for this room.
    pub notification_mode_user_defined: bool,
    pub is_favourite: bool,
    pub is_low_priority: bool,
    /// All tags currently set on the room, ordered as the server reports
    /// them (lexicographically by tag name).
    pub tags: Vec<MatrixRoomTag>,
    pub joined: bool,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixRoomNotificationMode {
    AllMessages,
    MentionsAndKeywordsOnly,
    Mute,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRoomTag {
    /// Full tag name, e.g. `m.favourite` or `u.work`.
    pub name: String,
    pub order: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetRoomNotificationModeRequest {
    pub room_id: String,
    /// `None` clears the user-defined rule so the room falls back to the
    /// room-type default (running keyword rules still apply).
    pub mode: Option<MatrixRoomNotificationMode>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetRoomNotificationModeResponse {
    pub set: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetRoomTagRequest {
    pub room_id: String,
    /// Full tag name, e.g. `u.work`. `m.favourite` and `m.lowpriority` are
    /// handled through the dedicated boolean flags instead.
    pub name: String,
    pub order: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetRoomTagResponse {
    pub set: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRemoveRoomTagRequest {
    pub room_id: String,
    pub name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRemoveRoomTagResponse {
    pub removed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetRoomFlagsRequest {
    pub room_id: String,
    pub is_favourite: bool,
    pub is_low_priority: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetRoomFlagsResponse {
    pub is_favourite: bool,
    pub is_low_priority: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixLeaveRoomRequest {
    pub room_id: String,
    /// Also ask the homeserver to forget the room after leaving.
    #[serde(default)]
    pub forget: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixLeaveRoomResponse {
    pub left: bool,
    pub forgotten: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixKeywordsResponse {
    pub keywords: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixAddKeywordRequest {
    pub keyword: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixAddKeywordResponse {
    pub keywords: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRemoveKeywordRequest {
    pub keyword: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRemoveKeywordResponse {
    pub keywords: Vec<String>,
}

/// Per-room keyword rules live as custom override push rules with the
/// `singularity.room_keyword.` rule-ID prefix; the keyword itself is
/// hex-encoded in the rule ID so any string is round-trippable.
pub const ROOM_KEYWORD_RULE_PREFIX: &str = "singularity.room_keyword.";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetRoomKeywordsRequest {
    pub room_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRoomKeywordsResponse {
    pub room_id: String,
    pub keywords: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixAddRoomKeywordRequest {
    pub room_id: String,
    pub keyword: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixAddRoomKeywordResponse {
    pub room_id: String,
    pub keywords: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRemoveRoomKeywordRequest {
    pub room_id: String,
    pub keyword: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRemoveRoomKeywordResponse {
    pub room_id: String,
    pub keywords: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixNotificationDefaultsResponse {
    pub encrypted: MatrixRoomNotificationMode,
    pub encrypted_one_to_one: MatrixRoomNotificationMode,
    pub unencrypted: MatrixRoomNotificationMode,
    pub unencrypted_one_to_one: MatrixRoomNotificationMode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetNotificationDefaultRequest {
    pub encrypted: bool,
    pub one_to_one: bool,
    pub mode: MatrixRoomNotificationMode,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetNotificationDefaultResponse {
    pub set: bool,
}
