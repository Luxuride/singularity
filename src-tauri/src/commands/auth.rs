//! Auth command adapters: sign-in (OAuth + password), session status, E2EE
//! recovery, logout, and cache clearing. Delegates to the `auth` crate.

use std::sync::Arc;

use tauri::State;

use auth::AuthState;
use storage::AppDb;
use types::auth::{
    MatrixAuthenticatedSessionResponse, MatrixClearCacheExceptAuthResponse,
    MatrixCompleteOAuthRequest, MatrixLogoutResponse, MatrixPasswordLoginRequest,
    MatrixRecoverWithKeyRequest, MatrixRecoverWithKeyResponse, MatrixRecoveryStatusResponse,
    MatrixSessionStatusResponse, MatrixStartOAuthRequest, MatrixStartOAuthResponse,
};
use types::Paths;

#[tauri::command]
pub async fn matrix_start_oauth(
    request: MatrixStartOAuthRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
) -> Result<MatrixStartOAuthResponse, String> {
    auth::start_oauth(&paths, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_complete_oauth(
    request: MatrixCompleteOAuthRequest,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
) -> Result<MatrixAuthenticatedSessionResponse, String> {
    auth::complete_oauth(&paths, &app_db, auth_state.as_ref(), &request.callback_url).await
}

#[tauri::command]
pub async fn matrix_password_login(
    request: MatrixPasswordLoginRequest,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
) -> Result<MatrixAuthenticatedSessionResponse, String> {
    auth::password_login(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_session_status(
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
) -> Result<MatrixSessionStatusResponse, String> {
    auth::session_status(&paths, &app_db, auth_state.as_ref()).await
}

#[tauri::command]
pub async fn matrix_recovery_status(
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
) -> Result<MatrixRecoveryStatusResponse, String> {
    auth::recovery_status(&paths, &app_db, auth_state.as_ref()).await
}

#[tauri::command]
pub async fn matrix_recover_with_key(
    request: MatrixRecoverWithKeyRequest,
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
) -> Result<MatrixRecoverWithKeyResponse, String> {
    auth::recover_with_key(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_logout(
    auth_state: State<'_, Arc<AuthState>>,
    app_db: State<'_, Arc<AppDb>>,
    paths: State<'_, Paths>,
) -> Result<MatrixLogoutResponse, String> {
    auth::logout(&paths, &app_db, auth_state.as_ref()).await
}

#[tauri::command]
pub fn matrix_clear_cache_except_auth(
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixClearCacheExceptAuthResponse, String> {
    auth::clear_cache_except_auth(&app_db)
}
