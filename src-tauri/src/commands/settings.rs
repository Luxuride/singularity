//! Settings command adapters: profile (display name, avatar) and account
//! (password change). Delegates to the `settings` crate.

use std::sync::Arc;

use tauri::State;

use auth::AuthState;
use storage::AppDb;
use types::settings::{
    MatrixChangePasswordRequest, MatrixChangePasswordResponse, MatrixProfileResponse,
    MatrixRemoveAvatarRequest, MatrixRemoveAvatarResponse, MatrixSetAvatarRequest,
    MatrixSetAvatarResponse, MatrixSetDisplayNameRequest, MatrixSetDisplayNameResponse,
};
use types::Paths;

#[tauri::command]
pub async fn matrix_get_profile(
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixProfileResponse, String> {
    settings::get_profile(&paths, &app_db, auth_state.as_ref()).await
}

#[tauri::command]
pub async fn matrix_set_display_name(
    request: MatrixSetDisplayNameRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSetDisplayNameResponse, String> {
    settings::set_display_name(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_set_avatar(
    request: MatrixSetAvatarRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSetAvatarResponse, String> {
    settings::set_avatar(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_remove_avatar(
    _request: MatrixRemoveAvatarRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixRemoveAvatarResponse, String> {
    settings::remove_avatar(&paths, &app_db, auth_state.as_ref()).await
}

#[tauri::command]
pub async fn matrix_change_password(
    request: MatrixChangePasswordRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixChangePasswordResponse, String> {
    settings::change_password(&paths, &app_db, auth_state.as_ref(), &request).await
}
