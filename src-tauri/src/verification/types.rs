use serde::Serialize;

/// Trust state for a specific other device as seen from this client, using
/// cross-signing trust that may propagate from other verified clients.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixDeviceTrust {
    /// Device is verified via cross-signing chain (may have been verified on
    /// another client and propagated here).
    CrossSigned,
    /// Device is verified only via a local trust flag set directly on this
    /// device.
    LocallyVerified,
    /// Device is known but not verified.
    NotVerified,
    /// Our own device, verification not applicable in the same way.
    OwnDevice,
}

/// A single device belonging to a Matrix user.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDeviceInfo {
    pub user_id: String,
    pub device_id: String,
    pub display_name: Option<String>,
    pub trust: MatrixDeviceTrust,
    /// Ed25519 fingerprint (base64), used to identify the device.
    pub ed25519_fingerprint: Option<String>,
}

/// Response for the get-user-devices command.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetUserDevicesResponse {
    pub user_id: String,
    /// Whether the user's identity is verified from our perspective.
    pub identity_verified: bool,
    pub devices: Vec<MatrixDeviceInfo>,
}

/// Top-level verification state of our own device / account.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixOwnVerificationStatus {
    pub user_id: String,
    pub device_id: String,
    /// Whether the local device is itself verified (signed by own Master Key).
    pub device_verified: bool,
    /// Whether cross-signing is set up for this account.
    pub cross_signing_setup: bool,
}

/// Result of triggering a verification request.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRequestVerificationResponse {
    /// Opaque flow identifier the frontend can pass back to track/accept the
    /// request.
    pub flow_id: String,
    pub user_id: String,
    pub device_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixVerificationRequestState {
    NotFound,
    Created,
    Requested,
    Ready,
    Transitioned,
    Done,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatrixSasVerificationState {
    Created,
    Started,
    Accepted,
    KeysExchanged,
    Confirmed,
    Done,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixVerificationEmoji {
    pub symbol: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixVerificationFlowResponse {
    pub flow_id: String,
    pub user_id: String,
    pub request_state: MatrixVerificationRequestState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sas_state: Option<MatrixSasVerificationState>,
    pub can_accept_request: bool,
    pub can_start_sas: bool,
    pub can_accept_sas: bool,
    pub can_confirm_sas: bool,
    pub is_done: bool,
    pub is_cancelled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<[u16; 3]>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub emojis: Vec<MatrixVerificationEmoji>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Emitted as a Tauri event when the own-device verification state changes.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixVerificationStateChangedEvent {
    pub verified: bool,
}
