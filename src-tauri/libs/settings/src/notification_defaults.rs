//! Global notification-mode defaults, applied by room type (encrypted /
//! one-to-one). These are the SDK's predefined underride rules, so they
//! apply to every room without a user-defined mode.

use std::sync::Arc;

use auth::state::AuthState;
use matrix_sdk::notification_settings::{IsEncrypted, IsOneToOne, RoomNotificationMode};
use storage::AppDb;
use types::settings::{
    MatrixNotificationDefaultsResponse, MatrixRoomNotificationMode,
    MatrixSetNotificationDefaultRequest, MatrixSetNotificationDefaultResponse,
};
use types::Paths;

use crate::commands::require_client;

fn mode_from_sdk(mode: RoomNotificationMode) -> MatrixRoomNotificationMode {
    match mode {
        RoomNotificationMode::AllMessages => MatrixRoomNotificationMode::AllMessages,
        RoomNotificationMode::MentionsAndKeywordsOnly => {
            MatrixRoomNotificationMode::MentionsAndKeywordsOnly
        }
        RoomNotificationMode::Mute => MatrixRoomNotificationMode::Mute,
    }
}

fn mode_to_sdk(mode: MatrixRoomNotificationMode) -> RoomNotificationMode {
    match mode {
        MatrixRoomNotificationMode::AllMessages => RoomNotificationMode::AllMessages,
        MatrixRoomNotificationMode::MentionsAndKeywordsOnly => {
            RoomNotificationMode::MentionsAndKeywordsOnly
        }
        MatrixRoomNotificationMode::Mute => RoomNotificationMode::Mute,
    }
}

async fn get_notification_defaults(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<MatrixNotificationDefaultsResponse, String> {
    let client = require_client(paths, app_db, auth_state).await?;
    let settings = client.notification_settings().await;

    Ok(MatrixNotificationDefaultsResponse {
        encrypted: mode_from_sdk(
            settings
                .get_default_room_notification_mode(IsEncrypted::Yes, IsOneToOne::No)
                .await,
        ),
        encrypted_one_to_one: mode_from_sdk(
            settings
                .get_default_room_notification_mode(IsEncrypted::Yes, IsOneToOne::Yes)
                .await,
        ),
        unencrypted: mode_from_sdk(
            settings
                .get_default_room_notification_mode(IsEncrypted::No, IsOneToOne::No)
                .await,
        ),
        unencrypted_one_to_one: mode_from_sdk(
            settings
                .get_default_room_notification_mode(IsEncrypted::No, IsOneToOne::Yes)
                .await,
        ),
    })
}

async fn set_notification_default(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixSetNotificationDefaultRequest,
) -> Result<MatrixSetNotificationDefaultResponse, String> {
    let client = require_client(paths, app_db, auth_state).await?;
    client
        .notification_settings()
        .await
        .set_default_room_notification_mode(
            IsEncrypted::from(request.encrypted),
            IsOneToOne::from(request.one_to_one),
            mode_to_sdk(request.mode),
        )
        .await
        .map_err(|error| format!("Failed to set notification default: {error}"))?;

    Ok(MatrixSetNotificationDefaultResponse { set: true })
}

pub(crate) mod delegates {
    //! Narrow re-exports so the crate root can expose one function per command.

    use super::*;

    pub async fn get_notification_defaults(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
    ) -> Result<MatrixNotificationDefaultsResponse, String> {
        super::get_notification_defaults(paths, app_db, auth_state).await
    }

    pub async fn set_notification_default(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixSetNotificationDefaultRequest,
    ) -> Result<MatrixSetNotificationDefaultResponse, String> {
        super::set_notification_default(paths, app_db, auth_state, request).await
    }
}
