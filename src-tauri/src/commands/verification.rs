//! Verification command adapters: own-device status, device listing, and the
//! interactive verification / SAS flows. Delegates to the `verification` crate.

use std::sync::Arc;

use tauri::State;

use auth::AuthState;
use storage::AppDb;
use types::verification::{
    MatrixGetUserDevicesRequest, MatrixGetUserDevicesResponse, MatrixOwnVerificationStatus,
    MatrixRequestDeviceVerificationRequest, MatrixRequestVerificationResponse,
    MatrixVerificationFlowRequest, MatrixVerificationFlowResponse,
};
use types::Paths;

#[tauri::command]
pub async fn matrix_own_verification_status(
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixOwnVerificationStatus, String> {
    verification::own_verification_status(&paths, &app_db, auth_state.as_ref()).await
}

#[tauri::command]
pub async fn matrix_get_user_devices(
    request: MatrixGetUserDevicesRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixGetUserDevicesResponse, String> {
    verification::get_user_devices(&paths, &app_db, auth_state.as_ref(), &request.user_id).await
}

#[tauri::command]
pub async fn matrix_request_device_verification(
    request: MatrixRequestDeviceVerificationRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixRequestVerificationResponse, String> {
    verification::request_device_verification(
        &paths,
        &app_db,
        auth_state.as_ref(),
        &request.user_id,
        &request.device_id,
    )
    .await
}

#[tauri::command]
pub async fn matrix_get_verification_flow(
    request: MatrixVerificationFlowRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixVerificationFlowResponse, String> {
    verification::get_verification_flow(
        &paths,
        &app_db,
        auth_state.as_ref(),
        &request.user_id,
        &request.flow_id,
    )
    .await
}

#[tauri::command]
pub async fn matrix_accept_verification_request(
    request: MatrixVerificationFlowRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixVerificationFlowResponse, String> {
    verification::accept_verification_request(
        &paths,
        &app_db,
        auth_state.as_ref(),
        &request.user_id,
        &request.flow_id,
    )
    .await
}

#[tauri::command]
pub async fn matrix_start_sas_verification(
    request: MatrixVerificationFlowRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixVerificationFlowResponse, String> {
    verification::start_sas_verification(
        &paths,
        &app_db,
        auth_state.as_ref(),
        &request.user_id,
        &request.flow_id,
    )
    .await
}

#[tauri::command]
pub async fn matrix_accept_sas_verification(
    request: MatrixVerificationFlowRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixVerificationFlowResponse, String> {
    verification::accept_sas_verification(
        &paths,
        &app_db,
        auth_state.as_ref(),
        &request.user_id,
        &request.flow_id,
    )
    .await
}

#[tauri::command]
pub async fn matrix_confirm_sas_verification(
    request: MatrixVerificationFlowRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixVerificationFlowResponse, String> {
    verification::confirm_sas_verification(
        &paths,
        &app_db,
        auth_state.as_ref(),
        &request.user_id,
        &request.flow_id,
    )
    .await
}
