use std::path::Path;

use log::warn;
use matrix_sdk::ruma::events::GlobalAccountDataEventType;
use tauri::{AppHandle, Emitter};

use crate::auth::AuthState;
use crate::db::AppDb;
use crate::messages::cache_mxc_media_to_local_path;
use crate::protocol::parse_room_id;
use crate::rooms::types::{
    MatrixGetChatsResponse, MatrixGetRoomImageRequest, MatrixGetRoomImageResponse,
};
use crate::rooms::RoomUpdateEvent;

use super::super::persistence::{load_cached_chats, store_cached_chats};

pub(super) fn has_stale_cached_chat_media(chats: &MatrixGetChatsResponse) -> bool {
    chats.chats.iter().any(|chat| {
        chat.image_url.as_deref().is_some_and(|url| {
            if url.starts_with("matrix-media://") {
                return true;
            }

            if url.starts_with('/') {
                return !Path::new(url).exists();
            }

            false
        })
    })
}

async fn direct_room_target_user_id(client: &matrix_sdk::Client, room_id: &str) -> Option<String> {
    let raw_content = client
        .account()
        .account_data_raw(GlobalAccountDataEventType::from("m.direct"))
        .await
        .ok()??;

    let content = raw_content.deserialize_as::<serde_json::Value>().ok()?;
    let mapping = content.as_object()?;
    let own_user_id = client.user_id().map(|value| value.as_str().to_string());

    for (user_id, room_ids) in mapping {
        if own_user_id.as_deref() == Some(user_id.as_str()) {
            continue;
        }

        let Some(room_ids) = room_ids.as_array() else {
            continue;
        };

        if room_ids
            .iter()
            .filter_map(|value| value.as_str())
            .any(|candidate_room_id| candidate_room_id == room_id)
        {
            return Some(user_id.to_string());
        }
    }

    None
}

async fn resolve_dm_avatar_source_url(
    client: &matrix_sdk::Client,
    room: &matrix_sdk::room::Room,
) -> Option<String> {
    let room_id = room.room_id().as_str();
    let dm_target_user_id = direct_room_target_user_id(client, room_id).await?;
    let dm_target_user_id =
        matrix_sdk::ruma::OwnedUserId::try_from(dm_target_user_id.as_str()).ok()?;

    match room.get_member(dm_target_user_id.as_ref()).await {
        Ok(Some(member)) => member.avatar_url().map(|avatar_url| avatar_url.to_string()),
        Ok(None) => None,
        Err(error) => {
            warn!(
                "Failed to fetch DM target member for avatar fallback in {}: {}",
                room_id, error
            );
            None
        }
    }
}

pub(super) async fn get_room_image(
    request: MatrixGetRoomImageRequest,
    auth_state: &AuthState,
    app_db: &AppDb,
    app_handle: &AppHandle,
) -> Result<MatrixGetRoomImageResponse, String> {
    if request.room_id.starts_with("virtual:") {
        return Ok(MatrixGetRoomImageResponse {
            room_id: request.room_id,
            image_url: None,
        });
    }

    let client = auth_state.restore_client_and_get(app_handle).await?;

    let room_id = parse_room_id(request.room_id.as_str())?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session yet"))?;

    let mut avatar_source_url = room.avatar_url().map(|mxc| mxc.to_string());
    if avatar_source_url.is_none() {
        avatar_source_url = resolve_dm_avatar_source_url(&client, &room).await;
    }

    let image_url = match avatar_source_url.as_deref() {
        Some(source_url) => cache_mxc_media_to_local_path(&client, source_url).await,
        None => None,
    };

    let _ = app_db.set_chat_image_source(request.room_id.as_str(), avatar_source_url.as_deref());

    if let Some(mut chats) = load_cached_chats(app_handle)? {
        if let Some(chat) = chats
            .iter_mut()
            .find(|candidate| candidate.room_id == request.room_id)
        {
            if chat.image_url != image_url {
                chat.image_url = image_url.clone();
                let updated_chat = chat.clone();

                let _ = store_cached_chats(app_handle, &chats);
                let _ = app_handle.emit(RoomUpdateEvent::RoomUpdated.as_str(), updated_chat);
            }
        }
    }

    Ok(MatrixGetRoomImageResponse {
        room_id: request.room_id,
        image_url,
    })
}

#[cfg(test)]
mod tests {
    use super::has_stale_cached_chat_media;
    use crate::rooms::types::{MatrixChatSummary, MatrixGetChatsResponse, MatrixRoomKind};

    fn chat_with_image(image_url: Option<&str>) -> MatrixChatSummary {
        MatrixChatSummary {
            room_id: String::from("!room:example.org"),
            display_name: String::from("Example"),
            image_url: image_url.map(ToOwned::to_owned),
            encrypted: false,
            joined_members: 2,
            kind: MatrixRoomKind::Room,
            joined: true,
            is_direct: false,
            join_rule: None,
            world_readable: None,
            guest_can_join: None,
            children_room_ids: vec![],
        }
    }

    #[test]
    fn detects_stale_matrix_media_avatar_url() {
        let response = MatrixGetChatsResponse {
            chats: vec![chat_with_image(Some(
                "matrix-media://localhost/img-123.png",
            ))],
        };

        assert!(has_stale_cached_chat_media(&response));
    }

    #[test]
    fn ignores_non_stale_avatar_urls() {
        let response = MatrixGetChatsResponse {
            chats: vec![
                chat_with_image(None),
                chat_with_image(Some("asset://localhost/home/user/.cache/eu.luxuride.singularity/media-cache/img-123.png")),
                chat_with_image(Some("https://example.org/avatar.png")),
            ],
        };

        assert!(!has_stale_cached_chat_media(&response));
    }
}
