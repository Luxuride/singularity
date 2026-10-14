//! Settings domain core functions: client restore and the domain-module
//! delegates used by the binder's command adapters.

use std::sync::Arc;

use auth::state::AuthState;
use matrix_sdk::ruma::api::client::uiaa::AuthData;
use matrix_sdk::Client;
use storage::AppDb;
use types::settings::{
    MatrixChangePasswordRequest, MatrixChangePasswordResponse, MatrixProfileResponse,
    MatrixRemoveAvatarResponse, MatrixSetAvatarRequest, MatrixSetAvatarResponse,
    MatrixSetDisplayNameRequest, MatrixSetDisplayNameResponse,
};
use types::Paths;

use crate::profile;

/// Restore the Matrix client from disk when needed, then hand it back.
pub async fn require_client(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<Client, String> {
    auth_state.restore_client_and_get(paths, app_db).await
}

pub async fn get_profile(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<MatrixProfileResponse, String> {
    profile::get_profile(paths, app_db, auth_state).await
}

pub async fn set_display_name(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixSetDisplayNameRequest,
) -> Result<MatrixSetDisplayNameResponse, String> {
    profile::set_display_name(paths, app_db, auth_state, request).await
}

pub async fn set_avatar(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    event_sink: &Arc<dyn types::EventSink>,
    request: &MatrixSetAvatarRequest,
) -> Result<MatrixSetAvatarResponse, String> {
    profile::set_avatar(paths, app_db, auth_state, event_sink, request).await
}

pub async fn remove_avatar(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    event_sink: &Arc<dyn types::EventSink>,
) -> Result<MatrixRemoveAvatarResponse, String> {
    profile::remove_avatar(paths, app_db, auth_state, event_sink).await
}

pub async fn change_password(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixChangePasswordRequest,
) -> Result<MatrixChangePasswordResponse, String> {
    if request.new_password.is_empty() {
        return Err(String::from("New password is required"));
    }

    let client = require_client(paths, app_db, auth_state).await?;

    // `Dummy` satisfies UIAa on homeservers that need no second factor.
    client
        .account()
        .change_password(
            &request.new_password,
            Some(AuthData::Dummy(
                matrix_sdk::ruma::api::client::uiaa::Dummy::new(),
            )),
        )
        .await
        .map_err(|error| format!("Failed to change password: {error}"))?;

    Ok(MatrixChangePasswordResponse { changed: true })
}

pub use crate::keywords::{add_keyword, get_keywords, remove_keyword};
pub use crate::notification_defaults::delegates::{
    get_notification_defaults, set_notification_default,
};
pub use crate::room_keywords::delegates::{
    add_room_keyword, get_room_keywords, remove_room_keyword,
};
pub use crate::room_settings::delegates::{
    get_room_settings as room_get_settings, leave_room as room_leave,
    remove_room_tag as room_remove_tag, set_room_flags as room_set_flags,
    set_room_notification_mode as room_set_notification_mode, set_room_tag as room_set_tag,
};
