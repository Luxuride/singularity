use log::{info, warn};
use tauri::{AppHandle, State};

use crate::auth::AuthState;

use super::super::helpers::{parse_room_id, parse_user_id};
use super::super::media::cache_mxc_media_to_local_path;
use super::super::types::{MatrixGetUserAvatarRequest, MatrixGetUserAvatarResponse};

#[tauri::command]
pub async fn matrix_get_user_avatar(
    request: MatrixGetUserAvatarRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetUserAvatarResponse, String> {
    info!("matrix_get_user_avatar requested");
    let client = auth_state.restore_client_and_get(&app_handle).await?;
    let user_id = parse_user_id(request.user_id.as_str())?;
    let room_id = parse_room_id(request.room_id.as_str())?;

    let room = match client.get_room(&room_id) {
        Some(room) => room,
        None => {
            return Ok(MatrixGetUserAvatarResponse {
                user_id: request.user_id,
                image_url: None,
            });
        }
    };

    let image_url = match room.get_member(user_id.as_ref()).await {
        Ok(Some(member)) => match member.avatar_url() {
            Some(avatar_url) => cache_mxc_media_to_local_path(&client, avatar_url.as_str()).await,
            None => None,
        },
        Ok(None) => None,
        Err(error) => {
            warn!(
                "Failed to fetch member profile for {} in {}: {}",
                user_id, room_id, error
            );
            None
        }
    };

    Ok(MatrixGetUserAvatarResponse {
        user_id: request.user_id,
        image_url,
    })
}
