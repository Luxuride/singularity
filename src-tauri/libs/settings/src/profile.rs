//! Profile read/write against the signed-in Matrix account.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use auth::AuthState;
use storage::AppDb;
use types::settings::{
    MatrixProfileResponse, MatrixRemoveAvatarResponse, MatrixSetAvatarRequest,
    MatrixSetAvatarResponse, MatrixSetDisplayNameRequest, MatrixSetDisplayNameResponse,
};
use types::Paths;

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
    let display_name = account
        .get_display_name()
        .await
        .map_err(|error| format!("Failed to get display name: {error}"))?;
    let avatar_mxc = account
        .get_avatar_url()
        .await
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

    Ok(MatrixSetAvatarResponse {
        mxc_url: mxc_url.to_string(),
    })
}

pub async fn remove_avatar(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<MatrixRemoveAvatarResponse, String> {
    let client = require_client(paths, app_db, auth_state).await?;

    client
        .account()
        .set_avatar_url(None)
        .await
        .map_err(|error| format!("Failed to remove avatar: {error}"))?;

    Ok(MatrixRemoveAvatarResponse { removed: true })
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
