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
