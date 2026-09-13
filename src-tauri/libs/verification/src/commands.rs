use log::{debug, warn};
use matrix_sdk::encryption::verification::{SasState, VerificationRequestState};
use matrix_sdk::encryption::VerificationState;
use matrix_sdk::ruma::{OwnedDeviceId, OwnedUserId};
use matrix_sdk::Client;
use std::sync::Arc;

use auth::AuthState;
use protocol::parse_user_id;
use protocol::sync::sync_once_serialized;
use storage::AppDb;
use types::config;
use types::event_paths::VERIFICATION_STATE_CHANGED;
use types::verification::{
    MatrixDeviceInfo, MatrixDeviceTrust, MatrixGetUserDevicesResponse, MatrixOwnVerificationStatus,
    MatrixRequestVerificationResponse, MatrixSasVerificationState, MatrixVerificationEmoji,
    MatrixVerificationFlowResponse, MatrixVerificationRequestState,
    MatrixVerificationStateChangedEvent,
};
use types::{EventSink, Paths};

async fn sync_after_verification(client: &Client) -> Result<(), String> {
    sync_once_serialized(
        client,
        matrix_sdk::config::SyncSettings::default()
            .timeout(std::time::Duration::from_secs(config::SYNC_TIMEOUT_SECONDS)),
    )
    .await
    .map_err(|error| format!("Failed to sync verification updates: {error}"))
}

async fn get_flow_response(
    client: &Client,
    user_id: &OwnedUserId,
    flow_id: &str,
) -> MatrixVerificationFlowResponse {
    let encryption = client.encryption();
    let request = encryption.get_verification_request(user_id, flow_id).await;
    let verification = encryption.get_verification(user_id, flow_id).await;

    let mut request_state = MatrixVerificationRequestState::NotFound;
    let mut sas_state = None;
    let mut can_accept_request = false;
    let mut can_start_sas = false;
    let mut can_accept_sas = false;
    let mut can_confirm_sas = false;
    let mut is_done = false;
    let mut is_cancelled = false;
    let mut decimals = None;
    let mut emojis = Vec::new();
    let mut message = None;

    if let Some(request) = request {
        match request.state() {
            VerificationRequestState::Created { .. } => {
                request_state = MatrixVerificationRequestState::Created;
            }
            VerificationRequestState::Requested { .. } => {
                request_state = MatrixVerificationRequestState::Requested;
                can_accept_request = true;
            }
            VerificationRequestState::Ready { .. } => {
                request_state = MatrixVerificationRequestState::Ready;
                can_start_sas = true;
            }
            VerificationRequestState::Transitioned { .. } => {
                request_state = MatrixVerificationRequestState::Transitioned;
            }
            VerificationRequestState::Done => {
                request_state = MatrixVerificationRequestState::Done;
                is_done = true;
            }
            VerificationRequestState::Cancelled(cancel_info) => {
                request_state = MatrixVerificationRequestState::Cancelled;
                is_cancelled = true;
                message = Some(cancel_info.reason().to_owned());
            }
        }
    }

    if let Some(sas) = verification.and_then(|verification| verification.sas()) {
        match sas.state() {
            SasState::Created { .. } => {
                sas_state = Some(MatrixSasVerificationState::Created);
            }
            SasState::Started { .. } => {
                sas_state = Some(MatrixSasVerificationState::Started);
                can_accept_sas = true;
            }
            SasState::Accepted { .. } => {
                sas_state = Some(MatrixSasVerificationState::Accepted);
            }
            SasState::KeysExchanged { .. } => {
                sas_state = Some(MatrixSasVerificationState::KeysExchanged);
                can_confirm_sas = true;
                decimals = sas
                    .decimals()
                    .map(|(first, second, third)| [first, second, third]);
                emojis = sas
                    .emoji()
                    .map(|values| {
                        values
                            .into_iter()
                            .map(|emoji| MatrixVerificationEmoji {
                                symbol: emoji.symbol.to_owned(),
                                description: emoji.description.to_owned(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            SasState::Confirmed => {
                sas_state = Some(MatrixSasVerificationState::Confirmed);
            }
            SasState::Done { .. } => {
                sas_state = Some(MatrixSasVerificationState::Done);
                is_done = true;
            }
            SasState::Cancelled(cancel_info) => {
                sas_state = Some(MatrixSasVerificationState::Cancelled);
                is_cancelled = true;
                message = Some(cancel_info.reason().to_owned());
            }
        }
    }

    MatrixVerificationFlowResponse {
        flow_id: flow_id.to_owned(),
        user_id: user_id.to_string(),
        request_state,
        sas_state,
        can_accept_request,
        can_start_sas,
        can_accept_sas,
        can_confirm_sas,
        is_done,
        is_cancelled,
        decimals,
        emojis,
        message,
    }
}

/// Return own device verification status and whether cross-signing is set up.
pub async fn own_verification_status(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<MatrixOwnVerificationStatus, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;

    let encryption = client.encryption();

    let own_device = encryption
        .get_own_device()
        .await
        .map_err(|e| format!("Failed to retrieve own device: {e}"))?;

    let (device_verified, cross_signing_setup, device_id) = match own_device {
        Some(device) => {
            let verified = device.is_verified();
            let cs_status = encryption.cross_signing_status().await;
            let setup = cs_status.map(|s| s.has_master).unwrap_or(false);
            (verified, setup, device.device_id().to_string())
        }
        None => (false, false, String::new()),
    };

    let user_id = client
        .user_id()
        .map(|id| id.to_string())
        .unwrap_or_default();

    Ok(MatrixOwnVerificationStatus {
        user_id,
        device_id,
        device_verified,
        cross_signing_setup,
    })
}

/// List all devices for a given user, with cross-signing trust state.
pub async fn get_user_devices(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    user_id_raw: &str,
) -> Result<MatrixGetUserDevicesResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;

    let user_id = parse_user_id(user_id_raw)?;

    let encryption = client.encryption();

    let user_devices = encryption
        .get_user_devices(&user_id)
        .await
        .map_err(|e| format!("Failed to get user devices: {e}"))?;

    let identity_verified = encryption
        .get_user_identity(&user_id)
        .await
        .map_err(|e| format!("Failed to get user identity: {e}"))?
        .map(|id| id.is_verified())
        .unwrap_or(false);

    let own_user_id = client
        .user_id()
        .map(|id| id.as_str().to_owned())
        .unwrap_or_default();
    let own_device_id = client
        .device_id()
        .map(|id| id.as_str().to_owned())
        .unwrap_or_default();

    let mut devices = Vec::new();
    for device in user_devices.devices() {
        let trust = if device.user_id().as_str() == own_user_id
            && device.device_id().as_str() == own_device_id
        {
            MatrixDeviceTrust::OwnDevice
        } else if device.is_verified_with_cross_signing() {
            MatrixDeviceTrust::CrossSigned
        } else if device.is_verified() {
            MatrixDeviceTrust::LocallyVerified
        } else {
            MatrixDeviceTrust::NotVerified
        };

        let ed25519_fingerprint = device.ed25519_key().map(|k| k.to_base64());

        devices.push(MatrixDeviceInfo {
            user_id: device.user_id().to_string(),
            device_id: device.device_id().to_string(),
            display_name: device.display_name().map(ToOwned::to_owned),
            trust,
            ed25519_fingerprint,
        });
    }

    debug!("Fetched {} devices for {}", devices.len(), user_id);

    Ok(MatrixGetUserDevicesResponse {
        user_id: user_id.to_string(),
        identity_verified,
        devices,
    })
}

/// Start an interactive verification request for a specific device.
pub async fn request_device_verification(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    user_id_raw: &str,
    device_id_raw: &str,
) -> Result<MatrixRequestVerificationResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;

    let user_id = parse_user_id(user_id_raw)?;
    let device_id = OwnedDeviceId::from(device_id_raw);

    let device = client
        .encryption()
        .get_device(&user_id, &device_id)
        .await
        .map_err(|e| format!("Failed to look up device: {e}"))?
        .ok_or_else(|| format!("Device {device_id_raw} not found for {user_id_raw}"))?;

    let request = device
        .request_verification()
        .await
        .map_err(|e| format!("Failed to request verification: {e}"))?;

    let flow_id = request.flow_id().to_owned();

    debug!("Verification request started: flow_id={flow_id}");

    Ok(MatrixRequestVerificationResponse {
        flow_id,
        user_id: user_id.to_string(),
        device_id: device_id.to_string(),
    })
}

/// Get the current state of a verification flow.
pub async fn get_verification_flow(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    user_id_raw: &str,
    flow_id: &str,
) -> Result<MatrixVerificationFlowResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;
    let user_id = parse_user_id(user_id_raw)?;

    Ok(get_flow_response(&client, &user_id, flow_id).await)
}

/// Accept a pending verification request.
pub async fn accept_verification_request(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    user_id_raw: &str,
    flow_id: &str,
) -> Result<MatrixVerificationFlowResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;
    let user_id = parse_user_id(user_id_raw)?;

    let request = client
        .encryption()
        .get_verification_request(&user_id, flow_id)
        .await
        .ok_or_else(|| format!("Verification request {flow_id} not found"))?;

    request
        .accept()
        .await
        .map_err(|error| format!("Failed to accept verification request: {error}"))?;

    Ok(get_flow_response(&client, &user_id, flow_id).await)
}

/// Start a SAS verification on a ready request.
pub async fn start_sas_verification(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    user_id_raw: &str,
    flow_id: &str,
) -> Result<MatrixVerificationFlowResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;
    let user_id = parse_user_id(user_id_raw)?;

    let request = client
        .encryption()
        .get_verification_request(&user_id, flow_id)
        .await
        .ok_or_else(|| format!("Verification request {flow_id} not found"))?;

    request
        .start_sas()
        .await
        .map_err(|error| format!("Failed to start SAS verification: {error}"))?
        .ok_or_else(|| String::from("Verification request is not ready to start SAS"))?;

    Ok(get_flow_response(&client, &user_id, flow_id).await)
}

/// Accept a started SAS verification.
pub async fn accept_sas_verification(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    user_id_raw: &str,
    flow_id: &str,
) -> Result<MatrixVerificationFlowResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;
    let user_id = parse_user_id(user_id_raw)?;

    let sas = client
        .encryption()
        .get_verification(&user_id, flow_id)
        .await
        .and_then(|verification| verification.sas())
        .ok_or_else(|| format!("SAS verification {flow_id} not found"))?;

    sas.accept()
        .await
        .map_err(|error| format!("Failed to accept SAS verification: {error}"))?;

    Ok(get_flow_response(&client, &user_id, flow_id).await)
}

/// Confirm a SAS verification (keys exchanged, user verified the emojis/decimals).
pub async fn confirm_sas_verification(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    user_id_raw: &str,
    flow_id: &str,
) -> Result<MatrixVerificationFlowResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;
    let user_id = parse_user_id(user_id_raw)?;

    let sas = client
        .encryption()
        .get_verification(&user_id, flow_id)
        .await
        .and_then(|verification| verification.sas())
        .ok_or_else(|| format!("SAS verification {flow_id} not found"))?;

    sas.confirm()
        .await
        .map_err(|error| format!("Failed to confirm SAS verification: {error}"))?;

    sync_after_verification(&client).await?;

    Ok(get_flow_response(&client, &user_id, flow_id).await)
}

/// Watch the SDK's own-device verification state in the background and emit
/// events to the frontend when it changes. Tauri-free: takes an
/// `Arc<dyn EventSink>` instead of an `AppHandle`.
pub fn start_verification_state_watcher(event_sink: Arc<dyn EventSink>, client: Client) {
    let mut sub = client.encryption().verification_state();

    tokio::spawn(async move {
        while let Some(state) = sub.next().await {
            let verified = state == VerificationState::Verified;
            let event = MatrixVerificationStateChangedEvent { verified };
            let payload = serde_json::to_value(&event).unwrap_or(serde_json::Value::Null);
            if let Err(e) = event_sink.emit(VERIFICATION_STATE_CHANGED, &payload) {
                warn!("Failed to emit verification state change: {e}");
            }
        }
    });
}
