//! Settings command adapters: profile (display name, avatar) and password
//! change. Delegates to the `settings` crate.

use std::sync::Arc;

use tauri::State;

use auth::AuthState;
use storage::AppDb;
use types::event_sink::EventSink;
use types::settings::{
    MatrixAddKeywordRequest, MatrixAddKeywordResponse, MatrixAddRoomKeywordRequest,
    MatrixAddRoomKeywordResponse, MatrixChangePasswordRequest, MatrixChangePasswordResponse,
    MatrixGetRoomKeywordsRequest, MatrixGetRoomSettingsRequest, MatrixKeywordsResponse,
    MatrixLeaveRoomRequest, MatrixLeaveRoomResponse, MatrixNotificationDefaultsResponse,
    MatrixProfileResponse, MatrixRemoveAvatarRequest, MatrixRemoveAvatarResponse,
    MatrixRemoveKeywordRequest, MatrixRemoveKeywordResponse, MatrixRemoveRoomKeywordRequest,
    MatrixRemoveRoomKeywordResponse, MatrixRemoveRoomTagRequest, MatrixRemoveRoomTagResponse,
    MatrixRoomKeywordsResponse, MatrixRoomSettingsResponse, MatrixSetAvatarRequest,
    MatrixSetAvatarResponse, MatrixSetDisplayNameRequest, MatrixSetDisplayNameResponse,
    MatrixSetNotificationDefaultRequest, MatrixSetNotificationDefaultResponse,
    MatrixSetRoomFlagsRequest, MatrixSetRoomFlagsResponse, MatrixSetRoomNotificationModeRequest,
    MatrixSetRoomNotificationModeResponse, MatrixSetRoomTagRequest, MatrixSetRoomTagResponse,
};
use types::{Paths, RoomUpdateTriggerState};

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
    event_sink: State<'_, Arc<dyn EventSink>>,
) -> Result<MatrixSetAvatarResponse, String> {
    settings::set_avatar(&paths, &app_db, auth_state.as_ref(), &event_sink, &request).await
}

#[tauri::command]
pub async fn matrix_remove_avatar(
    _request: MatrixRemoveAvatarRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
    event_sink: State<'_, Arc<dyn EventSink>>,
) -> Result<MatrixRemoveAvatarResponse, String> {
    settings::remove_avatar(&paths, &app_db, auth_state.as_ref(), &event_sink).await
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

#[tauri::command]
pub async fn matrix_get_keywords(
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixKeywordsResponse, String> {
    settings::get_keywords(&paths, &app_db, auth_state.as_ref()).await
}

#[tauri::command]
pub async fn matrix_add_keyword(
    request: MatrixAddKeywordRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixAddKeywordResponse, String> {
    settings::add_keyword(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_remove_keyword(
    request: MatrixRemoveKeywordRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixRemoveKeywordResponse, String> {
    settings::remove_keyword(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_room_get_keywords(
    request: MatrixGetRoomKeywordsRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixRoomKeywordsResponse, String> {
    settings::get_room_keywords(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_room_add_keyword(
    request: MatrixAddRoomKeywordRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixAddRoomKeywordResponse, String> {
    settings::add_room_keyword(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_room_remove_keyword(
    request: MatrixRemoveRoomKeywordRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixRemoveRoomKeywordResponse, String> {
    settings::remove_room_keyword(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_get_notification_defaults(
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixNotificationDefaultsResponse, String> {
    settings::get_notification_defaults(&paths, &app_db, auth_state.as_ref()).await
}

#[tauri::command]
pub async fn matrix_set_notification_default(
    request: MatrixSetNotificationDefaultRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSetNotificationDefaultResponse, String> {
    settings::set_notification_default(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_room_get_settings(
    request: MatrixGetRoomSettingsRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixRoomSettingsResponse, String> {
    settings::room_get_settings(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_room_set_notification_mode(
    request: MatrixSetRoomNotificationModeRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSetRoomNotificationModeResponse, String> {
    settings::room_set_notification_mode(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_room_set_tag(
    request: MatrixSetRoomTagRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSetRoomTagResponse, String> {
    settings::room_set_tag(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_room_remove_tag(
    request: MatrixRemoveRoomTagRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixRemoveRoomTagResponse, String> {
    settings::room_remove_tag(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_room_set_flags(
    request: MatrixSetRoomFlagsRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSetRoomFlagsResponse, String> {
    settings::room_set_flags(&paths, &app_db, auth_state.as_ref(), &request).await
}

#[tauri::command]
pub async fn matrix_room_leave(
    request: MatrixLeaveRoomRequest,
    auth_state: State<'_, Arc<AuthState>>,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<MatrixLeaveRoomResponse, String> {
    let response = settings::room_leave(&paths, &app_db, auth_state.as_ref(), &request).await;
    if response.is_ok() {
        let _ = trigger_state.enqueue_refresh(None, false);
    }
    response
}
