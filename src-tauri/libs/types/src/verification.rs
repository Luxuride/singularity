use serde::{Deserialize, Serialize};

/// Request to list a user's devices.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetUserDevicesRequest {
    pub user_id: String,
}

/// Request to start device verification with a specific device.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixRequestDeviceVerificationRequest {
    pub user_id: String,
    pub device_id: String,
}

/// Request to interact with an existing verification flow.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixVerificationFlowRequest {
    pub user_id: String,
    pub flow_id: String,
}

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
    /// Always serialized, including as an empty array, so the generated
    /// TypeScript type can mark it required.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(emojis: Vec<MatrixVerificationEmoji>) -> MatrixVerificationFlowResponse {
        MatrixVerificationFlowResponse {
            flow_id: "flow".to_owned(),
            user_id: "@alice:example.org".to_owned(),
            request_state: MatrixVerificationRequestState::Ready,
            sas_state: Some(MatrixSasVerificationState::Created),
            can_accept_request: true,
            can_start_sas: true,
            can_accept_sas: false,
            can_confirm_sas: false,
            is_done: false,
            is_cancelled: false,
            decimals: Some([1, 2, 3]),
            emojis,
            message: None,
        }
    }

    /// The generated TypeScript type declares `emojis` as required, so the
    /// wire format has to carry the key even when the list is empty.
    #[test]
    fn a_flow_without_emojis_still_serializes_the_key() {
        let json = serde_json::to_value(flow(Vec::new())).unwrap();

        assert_eq!(json["emojis"], serde_json::json!([]));
    }

    #[test]
    fn absent_optional_fields_are_omitted() {
        let mut response = flow(vec![MatrixVerificationEmoji {
            symbol: "\u{1f44b}".to_owned(),
            description: "wave".to_owned(),
        }]);
        response.decimals = None;
        response.message = None;

        let json = serde_json::to_value(&response).unwrap();

        assert!(json.get("decimals").is_none());
        assert!(json.get("message").is_none());
        assert_eq!(json["emojis"][0]["description"], "wave");
    }
}
