use serde::{Deserialize, Serialize};

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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCompleteOAuthRequest {
    pub callback_url: String,
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
pub struct MatrixRecoveryStatusResponse {
    pub state: String,
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
    pub state: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCompleteOAuthResponse {
    pub authenticated: bool,
    pub homeserver_url: String,
    pub user_id: String,
    pub device_id: String,
}
