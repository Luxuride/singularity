use serde::{Deserialize, Serialize};

/// The recovery state of the account's cross-signing keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixRecoveryState {
    Unknown,
    Enabled,
    Disabled,
    Incomplete,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixStartOAuthRequest {
    pub homeserver_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixStartOAuthResponse {
    pub authorization_url: String,
    pub redirect_uri: String,
    pub browser_opened: bool,
    pub deep_link_registered: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCompleteOAuthRequest {
    pub callback_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixPasswordLoginRequest {
    pub homeserver_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSessionStatusResponse {
    pub authenticated: bool,
    pub homeserver_url: Option<String>,
    pub user_id: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixLogoutResponse {
    pub logged_out: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixClearCacheExceptAuthResponse {
    pub cleared: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRecoveryStatusResponse {
    pub state: MatrixRecoveryState,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRecoverWithKeyRequest {
    pub recovery_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRecoverWithKeyResponse {
    pub recovered: bool,
    pub state: MatrixRecoveryState,
}

/// The authenticated session returned by both OAuth completion and password
/// login.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixAuthenticatedSessionResponse {
    pub authenticated: bool,
    pub homeserver_url: String,
    pub user_id: String,
    pub device_id: String,
}
