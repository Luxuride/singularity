//! Profile read/write against the signed-in Matrix account.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use auth::AuthState;
use protocol;
use rooms;
use storage::AppDb;
use types::event_paths;
use types::rooms::MatrixProfileAvatarUpdatedEvent;
use types::settings::{
    MatrixProfileResponse, MatrixRemoveAvatarResponse, MatrixSetAvatarRequest,
    MatrixSetAvatarResponse, MatrixSetDisplayNameRequest, MatrixSetDisplayNameResponse,
};
use types::{EventSink, Paths};

use crate::commands::require_client;

pub async fn get_profile(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<MatrixProfileResponse, String> {
    let client = require_client(paths, app_db, auth_state).await?;

    let user_id = client
        .user_id()
        .map(|id| id.to_string())
        .ok_or_else(|| String::from("Not authenticated"))?;

    let account = client.account();
    let (display_name, avatar_mxc) =
        tokio::join!(account.get_display_name(), account.get_avatar_url());
    let display_name =
        display_name.map_err(|error| format!("Failed to get display name: {error}"))?;
    let avatar_mxc = avatar_mxc
        .map_err(|error| format!("Failed to get avatar URL: {error}"))?
        .map(|mxc| mxc.to_string());

    // The frontend renders media from disk-backed `asset://` URLs.
    let avatar_url = match avatar_mxc.as_deref() {
        Some(mxc) => chat::cache_mxc_media_to_local_path(&client, mxc).await,
        None => None,
    };

    Ok(MatrixProfileResponse {
        user_id,
        display_name,
        avatar_url,
    })
}

pub async fn set_display_name(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixSetDisplayNameRequest,
) -> Result<MatrixSetDisplayNameResponse, String> {
    if request
        .display_name
        .as_deref()
        .is_some_and(|name| name.trim().is_empty())
    {
        return Err(String::from("Display name cannot be empty"));
    }

    let client = require_client(paths, app_db, auth_state).await?;
    let trimmed = request
        .display_name
        .as_deref()
        .map(|name| name.trim().to_owned());

    client
        .account()
        .set_display_name(trimmed.as_deref())
        .await
        .map_err(|error| format!("Failed to set display name: {error}"))?;

    Ok(MatrixSetDisplayNameResponse { set: true })
}

pub async fn set_avatar(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    event_sink: &Arc<dyn EventSink>,
    request: &MatrixSetAvatarRequest,
) -> Result<MatrixSetAvatarResponse, String> {
    let path = Path::new(&request.file_path);
    let bytes = fs::read(path).map_err(|error| format!("Failed to read avatar file: {error}"))?;

    if bytes.is_empty() {
        return Err(String::from("Avatar file is empty"));
    }

    if image::guess_format(&bytes).is_err() {
        return Err(String::from(
            "Avatar file is not a supported image (png, jpeg, gif, webp, bmp)",
        ));
    }

    let content_type = guess_image_mime(path)?;
    let client = require_client(paths, app_db, auth_state).await?;

    let mxc_url = client
        .account()
        .upload_avatar(&content_type, bytes)
        .await
        .map_err(|error| format!("Failed to upload avatar: {error}"))?;

    let mxc = mxc_url.to_string();
    apply_own_avatar_change(app_db, event_sink, &client, Some(&mxc)).await;

    Ok(MatrixSetAvatarResponse { mxc_url: mxc })
}

pub async fn remove_avatar(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    event_sink: &Arc<dyn EventSink>,
) -> Result<MatrixRemoveAvatarResponse, String> {
    let client = require_client(paths, app_db, auth_state).await?;

    client
        .account()
        .set_avatar_url(None)
        .await
        .map_err(|error| format!("Failed to remove avatar: {error}"))?;

    apply_own_avatar_change(app_db, event_sink, &client, None).await;

    Ok(MatrixRemoveAvatarResponse { removed: true })
}

/// Apply the just-made profile change locally instead of waiting for the sync
/// echo: record the new source, recache the media, refresh direct-room images
/// and tell the frontend whose avatars went stale. Mirrors what the
/// `rooms::avatars` watcher does when the change arrives over a later sync.
async fn apply_own_avatar_change(
    app_db: &Arc<AppDb>,
    event_sink: &Arc<dyn EventSink>,
    client: &matrix_sdk::Client,
    avatar_mxc: Option<&str>,
) {
    let Some(user_id) = client.user_id().map(|id| id.to_string()) else {
        return;
    };

    if let Err(error) = app_db.set_user_avatar_source(&user_id, avatar_mxc).await {
        log::warn!("Failed to record avatar source for {user_id}: {error}");
    }

    if let Some(mxc) = avatar_mxc {
        if chat::cache_mxc_media_to_local_path(client, mxc)
            .await
            .is_none()
        {
            log::warn!("Failed to cache avatar media for {mxc}");
        }
    }

    let deps = rooms::avatars::AvatarChangeDeps {
        app_db: app_db.clone(),
        event_sink: event_sink.clone(),
    };
    for (mapped_user_id, room_ids) in rooms::direct::direct_room_mapping(client).await {
        if mapped_user_id != user_id {
            continue;
        }
        for room_id in room_ids {
            let Ok(parsed_room_id) = protocol::parse_room_id(&room_id) else {
                continue;
            };
            let Some(room) = client.get_room(&parsed_room_id) else {
                continue;
            };
            rooms::avatars::refresh_room_image(&deps, &room, avatar_mxc.map(ToString::to_string))
                .await;
        }
    }

    let event = MatrixProfileAvatarUpdatedEvent { user_id };
    if let Ok(payload) = serde_json::to_value(&event) {
        let _ = event_sink.emit(event_paths::PROFILE_AVATAR_UPDATED, &payload);
    }
}

fn guess_image_mime(path: &Path) -> Result<mime::Mime, String> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());
    let extension = extension.as_deref();

    match extension {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("bmp") => "image/bmp",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/webp",
    }
    .parse::<mime::Mime>()
    .map_err(|error| format!("Invalid avatar mime type: {error}"))
}

#[cfg(test)]
mod tests {
    use super::guess_image_mime;
    use std::path::Path;

    #[test]
    fn guess_image_mime_maps_common_extensions() {
        for (name, expected) in [
            ("avatar.png", "image/png"),
            ("avatar.gif", "image/gif"),
            ("avatar.bmp", "image/bmp"),
            ("avatar.jpg", "image/jpeg"),
            ("avatar.jpeg", "image/jpeg"),
            ("avatar.webp", "image/webp"),
        ] {
            let mime = guess_image_mime(Path::new(name)).unwrap();
            assert_eq!(mime.essence_str(), expected, "extension mapping for {name}");
        }
    }

    #[test]
    fn guess_image_mime_falls_back_to_webp() {
        let mime = guess_image_mime(Path::new("avatar.svg")).unwrap();
        assert_eq!(mime.essence_str(), "image/webp");

        let mime = guess_image_mime(Path::new("no-extension")).unwrap();
        assert_eq!(mime.essence_str(), "image/webp");
    }

    #[test]
    fn guess_image_mime_is_case_insensitive() {
        let mime = guess_image_mime(Path::new("avatar.PNG")).unwrap();
        assert_eq!(mime.essence_str(), "image/png");
    }
}
