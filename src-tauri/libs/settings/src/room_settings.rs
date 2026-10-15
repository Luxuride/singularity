//! Per-room user settings: notification mode, favourite/low-priority flags,
//! arbitrary user tags, and leave/forget. Acts only on the caller's own
//! membership and view of the room, never on power levels.

use std::sync::Arc;

use auth::AuthState;
use matrix_sdk::ruma::events::tag::{InvalidUserTagName, TagInfo, TagName};
use matrix_sdk::ruma::RoomId;
use matrix_sdk::RoomState;
use storage::AppDb;
use types::settings::{
    MatrixGetRoomSettingsRequest, MatrixLeaveRoomRequest, MatrixLeaveRoomResponse,
    MatrixRemoveRoomTagRequest, MatrixRemoveRoomTagResponse, MatrixRoomNotificationMode,
    MatrixRoomSettingsResponse, MatrixRoomTag, MatrixSetRoomFlagsRequest,
    MatrixSetRoomFlagsResponse, MatrixSetRoomNotificationModeRequest,
    MatrixSetRoomNotificationModeResponse, MatrixSetRoomTagRequest, MatrixSetRoomTagResponse,
};
use types::Paths;

use crate::commands::require_client;
use crate::room_keywords::{
    delete_room_keyword_rules, recreate_room_keywords, room_keywords_from_ruleset,
};

fn matrix_room_kind(room: &matrix_sdk::room::Room) -> types::rooms::MatrixRoomKind {
    if room.is_space() {
        types::rooms::MatrixRoomKind::Space
    } else {
        types::rooms::MatrixRoomKind::Room
    }
}

fn notification_mode_from_sdk(
    mode: matrix_sdk::notification_settings::RoomNotificationMode,
) -> MatrixRoomNotificationMode {
    match mode {
        matrix_sdk::notification_settings::RoomNotificationMode::AllMessages => {
            MatrixRoomNotificationMode::AllMessages
        }
        matrix_sdk::notification_settings::RoomNotificationMode::MentionsAndKeywordsOnly => {
            MatrixRoomNotificationMode::MentionsAndKeywordsOnly
        }
        matrix_sdk::notification_settings::RoomNotificationMode::Mute => {
            MatrixRoomNotificationMode::Mute
        }
    }
}

fn notification_mode_to_sdk(
    mode: MatrixRoomNotificationMode,
) -> matrix_sdk::notification_settings::RoomNotificationMode {
    match mode {
        MatrixRoomNotificationMode::AllMessages => {
            matrix_sdk::notification_settings::RoomNotificationMode::AllMessages
        }
        MatrixRoomNotificationMode::MentionsAndKeywordsOnly => {
            matrix_sdk::notification_settings::RoomNotificationMode::MentionsAndKeywordsOnly
        }
        MatrixRoomNotificationMode::Mute => {
            matrix_sdk::notification_settings::RoomNotificationMode::Mute
        }
    }
}

fn parse_tag_name(name: &str) -> Result<TagName, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(String::from("Tag name is required"));
    }

    if trimmed.starts_with("u.") {
        let user_tag = std::str::FromStr::from_str(trimmed)
            .map_err(|_: InvalidUserTagName| format!("Invalid user tag name `{trimmed}`"))?;
        return Ok(TagName::User(user_tag));
    }

    Ok(TagName::from(trimmed.to_owned()))
}

fn tag_name_to_string(tag: &TagName) -> String {
    tag.as_ref().to_owned()
}

async fn get_room_settings(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    room_id_raw: &str,
) -> Result<MatrixRoomSettingsResponse, String> {
    let room_id = RoomId::parse(room_id_raw)
        .map_err(|error| format!("Invalid room ID `{room_id_raw}`: {error}"))?;

    let client = require_client(paths, app_db, auth_state).await?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| format!("Room {room_id} not found"))?;

    let kind = matrix_room_kind(&room);
    let joined = room.state() == RoomState::Joined;

    let tags = room
        .tags()
        .await
        .map_err(|error| format!("Failed to load room tags: {error}"))?
        .unwrap_or_default()
        .into_iter()
        .map(|(name, info)| MatrixRoomTag {
            name: tag_name_to_string(&name),
            order: info.order,
        })
        .collect();

    let notification_mode = if joined {
        room.notification_mode().await
    } else {
        None
    };

    let notification_mode_user_defined = if joined {
        client
            .notification_settings()
            .await
            .get_user_defined_room_notification_mode(&room_id)
            .await
            .is_some()
    } else {
        false
    };

    Ok(MatrixRoomSettingsResponse {
        room_id: room_id.to_string(),
        kind,
        notification_mode: notification_mode.map(notification_mode_from_sdk),
        notification_mode_user_defined,
        is_favourite: room.is_favourite(),
        is_low_priority: room.is_low_priority(),
        tags,
        joined,
    })
}

async fn set_room_notification_mode(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    room_id_raw: &str,
    mode: Option<MatrixRoomNotificationMode>,
) -> Result<MatrixSetRoomNotificationModeResponse, String> {
    let room_id = RoomId::parse(room_id_raw)
        .map_err(|error| format!("Invalid room ID `{room_id_raw}`: {error}"))?;

    let client = require_client(paths, app_db, auth_state).await?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| format!("Room {room_id} not found"))?;
    if room.state() != RoomState::Joined {
        return Err(format!("Room {room_id} is not joined"));
    }

    let settings = client.notification_settings().await;

    // Both `set_room_notification_mode` and `delete_user_defined_room_rules`
    // wipe every other custom rule for the room, including the keyword
    // rules, so they must be re-created afterwards.
    let ruleset = settings.ruleset().await;
    let keywords = room_keywords_from_ruleset(&ruleset, &room_id);

    let result = match mode {
        Some(mode) => {
            settings
                .set_room_notification_mode(&room_id, notification_mode_to_sdk(mode))
                .await
        }
        None => settings.delete_user_defined_room_rules(&room_id).await,
    };

    result.map_err(|error| format!("Failed to update notification mode: {error}"))?;

    if !keywords.is_empty() {
        recreate_room_keywords(&settings, &room_id, &keywords).await?;
    }

    Ok(MatrixSetRoomNotificationModeResponse { set: true })
}

async fn set_room_tag(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    room_id_raw: &str,
    tag_name_raw: &str,
    order: Option<f64>,
) -> Result<MatrixSetRoomTagResponse, String> {
    let room_id = RoomId::parse(room_id_raw)
        .map_err(|error| format!("Invalid room ID `{room_id_raw}`: {error}"))?;
    let tag = parse_tag_name(tag_name_raw)?;

    let client = require_client(paths, app_db, auth_state).await?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| format!("Room {room_id} not found"))?;

    let mut tag_info = TagInfo::new();
    tag_info.order = order;

    room.set_tag(tag, tag_info)
        .await
        .map_err(|error| format!("Failed to set tag: {error}"))?;

    Ok(MatrixSetRoomTagResponse { set: true })
}

async fn remove_room_tag(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    room_id_raw: &str,
    tag_name_raw: &str,
) -> Result<MatrixRemoveRoomTagResponse, String> {
    let room_id = RoomId::parse(room_id_raw)
        .map_err(|error| format!("Invalid room ID `{room_id_raw}`: {error}"))?;
    let tag = parse_tag_name(tag_name_raw)?;

    let client = require_client(paths, app_db, auth_state).await?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| format!("Room {room_id} not found"))?;

    room.remove_tag(tag)
        .await
        .map_err(|error| format!("Failed to remove tag: {error}"))?;

    Ok(MatrixRemoveRoomTagResponse { removed: true })
}

async fn set_room_flags(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixSetRoomFlagsRequest,
) -> Result<MatrixSetRoomFlagsResponse, String> {
    let room_id = RoomId::parse(&request.room_id)
        .map_err(|error| format!("Invalid room ID `{}`: {error}", request.room_id))?;

    let client = require_client(paths, app_db, auth_state).await?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| format!("Room {room_id} not found"))?;

    if request.is_favourite {
        room.set_is_favourite(true, None)
            .await
            .map_err(|error| format!("Failed to set favourite: {error}"))?;
    } else {
        room.set_is_favourite(false, None)
            .await
            .map_err(|error| format!("Failed to unset favourite: {error}"))?;
    }

    if request.is_low_priority {
        room.set_is_low_priority(true, None)
            .await
            .map_err(|error| format!("Failed to set low priority: {error}"))?;
    } else {
        room.set_is_low_priority(false, None)
            .await
            .map_err(|error| format!("Failed to unset low priority: {error}"))?;
    }

    Ok(MatrixSetRoomFlagsResponse {
        is_favourite: request.is_favourite,
        is_low_priority: request.is_low_priority,
    })
}

async fn leave_room(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixLeaveRoomRequest,
) -> Result<MatrixLeaveRoomResponse, String> {
    let room_id = RoomId::parse(&request.room_id)
        .map_err(|error| format!("Invalid room ID `{}`: {error}", request.room_id))?;

    let client = require_client(paths, app_db, auth_state).await?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| format!("Room {room_id} not found"))?;

    let state = room.state();
    if state != RoomState::Joined && state != RoomState::Invited {
        return Err(format!(
            "Room {room_id} cannot be left from state {state:?}"
        ));
    }

    room.leave()
        .await
        .map_err(|error| format!("Failed to leave room: {error}"))?;

    // Keyword rules are per-room push rules, not room account data, so the
    // homeserver keeps them after leaving; remove them explicitly.
    let settings = client.notification_settings().await;
    delete_room_keyword_rules(&settings, &client, &room_id)
        .await
        .map_err(|error| format!("Failed to clean up room keywords: {error}"))?;

    let mut forgotten = false;
    if request.forget {
        // `forget` only accepts Left/Banned rooms, so it must run after the
        // leave completes on the server.
        let left_room = client
            .get_room(&room_id)
            .ok_or_else(|| format!("Room {room_id} not found after leaving"))?;
        match left_room.forget().await {
            Ok(()) => forgotten = true,
            Err(error) => return Err(format!("Failed to forget room: {error}")),
        }
    }

    Ok(MatrixLeaveRoomResponse {
        left: true,
        forgotten,
    })
}

pub(crate) mod delegates {
    //! Narrow re-exports so the crate root can expose one function per command.

    use super::*;

    pub async fn get_room_settings(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixGetRoomSettingsRequest,
    ) -> Result<MatrixRoomSettingsResponse, String> {
        super::get_room_settings(paths, app_db, auth_state, &request.room_id).await
    }

    pub async fn set_room_notification_mode(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixSetRoomNotificationModeRequest,
    ) -> Result<MatrixSetRoomNotificationModeResponse, String> {
        super::set_room_notification_mode(paths, app_db, auth_state, &request.room_id, request.mode)
            .await
    }

    pub async fn set_room_tag(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixSetRoomTagRequest,
    ) -> Result<MatrixSetRoomTagResponse, String> {
        super::set_room_tag(
            paths,
            app_db,
            auth_state,
            &request.room_id,
            &request.name,
            request.order,
        )
        .await
    }

    pub async fn remove_room_tag(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixRemoveRoomTagRequest,
    ) -> Result<MatrixRemoveRoomTagResponse, String> {
        super::remove_room_tag(paths, app_db, auth_state, &request.room_id, &request.name).await
    }

    pub async fn set_room_flags(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixSetRoomFlagsRequest,
    ) -> Result<MatrixSetRoomFlagsResponse, String> {
        super::set_room_flags(paths, app_db, auth_state, request).await
    }

    pub async fn leave_room(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixLeaveRoomRequest,
    ) -> Result<MatrixLeaveRoomResponse, String> {
        super::leave_room(paths, app_db, auth_state, request).await
    }
}
