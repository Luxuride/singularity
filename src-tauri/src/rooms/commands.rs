use std::collections::{HashMap, HashSet};
use std::path::Path;
use log::warn;
use matrix_sdk::ruma::events::GlobalAccountDataEventType;
use tauri::{AppHandle, Emitter, State};

use crate::auth::AuthState;
use crate::db::AppDb;
use crate::messages::cache_mxc_media_to_local_path;

use super::persistence::{collect_and_store_chats, load_cached_chats, store_cached_chats};
use super::types::{
    MatrixChatSummary, MatrixGetChatNavigationRequest, MatrixGetChatNavigationResponse,
    MatrixGetChatsResponse, MatrixGetRoomImageRequest, MatrixGetRoomImageResponse, MatrixRoomKind,
};
use super::{
    MatrixTriggerRoomUpdateRequest, MatrixTriggerRoomUpdateResponse, RoomRefreshTrigger,
    RoomUpdateEvent, RoomUpdateTriggerState,
};

const VIRTUAL_DMS_ROOT_ID: &str = "virtual:dms";
const VIRTUAL_UNSPACED_ROOT_ID: &str = "virtual:unspaced";

struct NavigationIndex<'a> {
    chats: &'a [MatrixChatSummary],
    room_by_id: HashMap<&'a str, &'a MatrixChatSummary>,
    parent_ids_by_room: HashMap<&'a str, Vec<&'a str>>,
    children_by_parent: HashMap<&'a str, Vec<&'a str>>,
}

impl<'a> NavigationIndex<'a> {
    fn new(chats: &'a [MatrixChatSummary]) -> Self {
        let room_by_id = chats
            .iter()
            .map(|room| (room.room_id.as_str(), room))
            .collect::<HashMap<_, _>>();

        let mut parent_ids_by_room = HashMap::<&str, Vec<&str>>::new();
        let mut children_by_parent = HashMap::<&str, Vec<&str>>::new();

        for room in chats {
            let mut children = Vec::<&str>::new();

            for child_room_id in &room.children_room_ids {
                let child_room_id = child_room_id.as_str();
                if child_room_id == room.room_id || !room_by_id.contains_key(child_room_id) {
                    continue;
                }

                children.push(child_room_id);
                parent_ids_by_room
                    .entry(child_room_id)
                    .or_default()
                    .push(room.room_id.as_str());
            }

            children.sort_unstable();
            children_by_parent.insert(room.room_id.as_str(), children);
        }

        for parents in parent_ids_by_room.values_mut() {
            parents.sort_unstable();
        }

        Self {
            chats,
            room_by_id,
            parent_ids_by_room,
            children_by_parent,
        }
    }

    fn chat(&self, room_id: &str) -> Option<&MatrixChatSummary> {
        self.room_by_id.get(room_id).copied()
    }

    fn parent_ids(&self, room_id: &str) -> &[&str] {
        self.parent_ids_by_room
            .get(room_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn is_unspaced_room(&self, room: &MatrixChatSummary) -> bool {
        if room.kind != MatrixRoomKind::Room || room.is_direct {
            return false;
        }

        self.parent_ids(room.room_id.as_str()).is_empty()
    }

    fn root_parent_space_id(&self, room: &MatrixChatSummary) -> Option<String> {
        let mut stack = self.parent_ids(room.room_id.as_str()).to_vec();
        let mut seen = HashSet::<&str>::new();

        while let Some(parent_id) = stack.pop() {
            if !seen.insert(parent_id) {
                continue;
            }

            let Some(parent_room) = self.chat(parent_id) else {
                continue;
            };

            let parent_parents = self.parent_ids(parent_room.room_id.as_str());
            if parent_parents.is_empty() {
                return Some(parent_room.room_id.clone());
            }

            for ancestor_parent_id in parent_parents.iter().rev() {
                stack.push(*ancestor_parent_id);
            }
        }

        None
    }

    fn is_root_space(&self, room_id: &str) -> bool {
        if room_id == VIRTUAL_DMS_ROOT_ID || room_id == VIRTUAL_UNSPACED_ROOT_ID {
            return true;
        }

        let Some(room) = self.chat(room_id) else {
            return false;
        };

        room.kind == MatrixRoomKind::Space && self.parent_ids(room.room_id.as_str()).is_empty()
    }

    fn derive_root_space_id_for_room(&self, room_id: &str) -> Option<String> {
        let current = self.chat(room_id);

        if let Some(room) = current {
            if room.kind == MatrixRoomKind::Room && room.is_direct {
                return Some(VIRTUAL_DMS_ROOT_ID.to_string());
            }

            if self.is_unspaced_room(room) {
                return Some(VIRTUAL_UNSPACED_ROOT_ID.to_string());
            }
        }

        if let Some(room) = current {
            if room.kind == MatrixRoomKind::Space
                && self.parent_ids(room.room_id.as_str()).is_empty()
            {
                return Some(room.room_id.clone());
            }

            if let Some(root_parent_space_id) = self.root_parent_space_id(room) {
                return Some(root_parent_space_id);
            }
        }

        None
    }

    fn build_root_spaces(&self) -> Vec<MatrixChatSummary> {
        let direct_count = self
            .chats
            .iter()
            .filter(|room| room.kind == MatrixRoomKind::Room && room.is_direct)
            .count() as u64;
        let unspaced_count = self
            .chats
            .iter()
            .filter(|room| self.is_unspaced_room(room))
            .count() as u64;

        let dms_root = MatrixChatSummary {
            room_id: VIRTUAL_DMS_ROOT_ID.to_string(),
            display_name: String::from("DMs"),
            image_url: None,
            encrypted: false,
            joined_members: direct_count,
            kind: MatrixRoomKind::Space,
            joined: true,
            is_direct: false,
            children_room_ids: vec![],
        };

        let unspaced_root = MatrixChatSummary {
            room_id: VIRTUAL_UNSPACED_ROOT_ID.to_string(),
            display_name: String::from("Rooms"),
            image_url: None,
            encrypted: false,
            joined_members: unspaced_count,
            kind: MatrixRoomKind::Space,
            joined: true,
            is_direct: false,
            children_room_ids: vec![],
        };

        let mut matrix_root_spaces = self
            .chats
            .iter()
            .filter(|room| room.kind == MatrixRoomKind::Space)
            .filter(|space| self.parent_ids(space.room_id.as_str()).is_empty())
            .cloned()
            .collect::<Vec<_>>();
        sort_rooms_by_display_name(&mut matrix_root_spaces);

        let mut roots = Vec::with_capacity(matrix_root_spaces.len() + 3);
        roots.push(dms_root);
        roots.push(unspaced_root);
        roots.extend(matrix_root_spaces);
        roots
    }

    fn build_root_scoped_rooms(&self, root_space_id: &str) -> Vec<MatrixChatSummary> {
        if root_space_id == VIRTUAL_DMS_ROOT_ID {
            let mut rooms = self
                .chats
                .iter()
                .filter(|room| room.kind == MatrixRoomKind::Room && room.is_direct)
                .cloned()
                .collect::<Vec<_>>();
            sort_rooms_by_display_name(&mut rooms);
            return rooms;
        }

        if root_space_id == VIRTUAL_UNSPACED_ROOT_ID {
            let mut rooms = self
                .chats
                .iter()
                .filter(|room| self.is_unspaced_room(room))
                .cloned()
                .collect::<Vec<_>>();
            sort_rooms_by_display_name(&mut rooms);
            return rooms;
        }

        let mut descendants = Vec::<MatrixChatSummary>::new();
        let mut stack: Vec<(&str, HashSet<&str>)> = self
            .children_by_parent
            .get(root_space_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|child_id| (child_id, HashSet::new()))
            .collect();

        while let Some((room_id, ancestry)) = stack.pop() {
            if ancestry.contains(room_id) {
                continue;
            }

            let Some(candidate) = self.chat(room_id) else {
                continue;
            };

            descendants.push(candidate.clone());

            let mut next_ancestry = ancestry;
            next_ancestry.insert(room_id);

            if let Some(children) = self.children_by_parent.get(room_id) {
                for child_id in children {
                    stack.push((child_id, next_ancestry.clone()));
                }
            }
        }

        sort_rooms_by_display_name(&mut descendants);
        descendants
    }
}

fn has_stale_cached_chat_media(chats: &MatrixGetChatsResponse) -> bool {
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

#[tauri::command]
pub async fn matrix_get_chats(
    auth_state: State<'_, AuthState>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatsResponse, String> {
    if let Some(cached_chats) = load_cached_chats(&app_handle)? {
        let cached = MatrixGetChatsResponse {
            chats: cached_chats,
        };

        if has_stale_cached_chat_media(&cached) {
            let client = auth_state.restore_client_and_get(&app_handle).await?;

            let local_chats = collect_and_store_chats(&app_handle, &client).await;
            if !local_chats.is_empty() {
                let _ = trigger_state.enqueue(RoomRefreshTrigger {
                    selected_room_id: None,
                    include_selected_messages: false,
                });

                return Ok(MatrixGetChatsResponse { chats: local_chats });
            }
        }

        let _ = trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: None,
            include_selected_messages: false,
        });

        return Ok(cached);
    }

    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let local_chats = collect_and_store_chats(&app_handle, &client).await;
    if !local_chats.is_empty() {
        let _ = trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: None,
            include_selected_messages: false,
        });

        return Ok(MatrixGetChatsResponse { chats: local_chats });
    }

    let _ = trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: None,
        include_selected_messages: false,
    });

    Ok(MatrixGetChatsResponse { chats: local_chats })
}

#[tauri::command]
pub fn matrix_get_chat_navigation(
    request: Option<MatrixGetChatNavigationRequest>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatNavigationResponse, String> {
    let payload = request.unwrap_or_default();
    let chats = load_cached_chats(&app_handle)?.unwrap_or_default();
    let index = NavigationIndex::new(&chats);

    let selected_root_space_id = choose_selected_root_space_id(
        &index,
        payload.root_space_id.as_deref(),
        payload.selected_room_id.as_deref(),
    );

    let root_spaces = index.build_root_spaces();
    let root_scoped_rooms = selected_root_space_id
        .as_deref()
        .map(|root_space_id| index.build_root_scoped_rooms(root_space_id))
        .unwrap_or_default();

    Ok(MatrixGetChatNavigationResponse {
        selected_root_space_id,
        root_spaces,
        root_scoped_rooms,
    })
}

#[tauri::command]
pub async fn matrix_trigger_room_update(
    request: Option<MatrixTriggerRoomUpdateRequest>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<MatrixTriggerRoomUpdateResponse, String> {
    let payload = request.unwrap_or_default();

    trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: payload.selected_room_id,
        include_selected_messages: payload.include_selected_messages,
    })?;

    Ok(MatrixTriggerRoomUpdateResponse { queued: true })
}

#[tauri::command]
pub async fn matrix_get_room_image(
    request: MatrixGetRoomImageRequest,
    auth_state: State<'_, AuthState>,
    app_db: State<'_, AppDb>,
    app_handle: AppHandle,
) -> Result<MatrixGetRoomImageResponse, String> {
    if request.room_id.starts_with("virtual:") {
        return Ok(MatrixGetRoomImageResponse {
            room_id: request.room_id,
            image_url: None,
        });
    }

    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(request.room_id.as_str())
        .map_err(|_| String::from("roomId is invalid"))?;
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

    if let Some(mut chats) = load_cached_chats(&app_handle)? {
        if let Some(chat) = chats
            .iter_mut()
            .find(|candidate| candidate.room_id == request.room_id)
        {
            if chat.image_url != image_url {
                chat.image_url = image_url.clone();
                let updated_chat = chat.clone();

                let _ = store_cached_chats(&app_handle, &chats);
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
fn build_root_spaces(chats: &[MatrixChatSummary]) -> Vec<MatrixChatSummary> {
    NavigationIndex::new(chats).build_root_spaces()
}

fn sort_rooms_by_display_name(rooms: &mut [MatrixChatSummary]) {
    rooms.sort_by_cached_key(|room| room.display_name.to_lowercase());
}

fn is_root_space(room_id: &str, index: &NavigationIndex<'_>) -> bool {
    index.is_root_space(room_id)
}

#[cfg(test)]
fn derive_root_space_id_for_room(room_id: &str, chats: &[MatrixChatSummary]) -> Option<String> {
    NavigationIndex::new(chats).derive_root_space_id_for_room(room_id)
}

fn choose_selected_root_space_id(
    index: &NavigationIndex<'_>,
    requested_root_space_id: Option<&str>,
    selected_room_id: Option<&str>,
) -> Option<String> {
    if let Some(root_space_id) = requested_root_space_id {
        if is_root_space(root_space_id, index) {
            return Some(root_space_id.to_string());
        }
    }

    selected_room_id.and_then(|room_id| index.derive_root_space_id_for_room(room_id))
}

#[cfg(test)]
fn build_root_scoped_rooms(
    chats: &[MatrixChatSummary],
    root_space_id: &str,
) -> Vec<MatrixChatSummary> {
    NavigationIndex::new(chats).build_root_scoped_rooms(root_space_id)
}

#[cfg(test)]
mod tests {
    use super::{
        build_root_scoped_rooms, build_root_spaces, derive_root_space_id_for_room,
        has_stale_cached_chat_media, VIRTUAL_DMS_ROOT_ID, VIRTUAL_UNSPACED_ROOT_ID,
    };
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
                chat_with_image(Some("asset://localhost/tmp/img-123.png")),
                chat_with_image(Some("https://example.org/avatar.png")),
            ],
        };

        assert!(!has_stale_cached_chat_media(&response));
    }

    fn chat(
        room_id: &str,
        kind: MatrixRoomKind,
        is_direct: bool,
        children_room_ids: &[&str],
    ) -> MatrixChatSummary {
        MatrixChatSummary {
            room_id: room_id.to_string(),
            display_name: room_id.to_string(),
            image_url: None,
            encrypted: false,
            joined_members: 0,
            kind,
            joined: true,
            is_direct,
            children_room_ids: children_room_ids
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        }
    }

    #[test]
    fn builds_virtual_roots_with_counts() {
        let chats = vec![
            chat("!dm:example.org", MatrixRoomKind::Room, true, &[]),
            chat("!orphan:example.org", MatrixRoomKind::Room, false, &[]),
            chat(
                "!space:example.org",
                MatrixRoomKind::Space,
                false,
                &["!child:example.org"],
            ),
            chat("!child:example.org", MatrixRoomKind::Room, false, &[]),
        ];

        let roots = build_root_spaces(&chats);
        assert_eq!(roots[0].room_id, VIRTUAL_DMS_ROOT_ID);
        assert_eq!(roots[0].joined_members, 1);
        assert_eq!(roots[1].room_id, VIRTUAL_UNSPACED_ROOT_ID);
        assert_eq!(roots[1].joined_members, 1);
        assert_eq!(roots[2].room_id, "!space:example.org");
    }

    #[test]
    fn derives_virtual_root_for_unspaced_room() {
        let chats = vec![
            chat("!dm:example.org", MatrixRoomKind::Room, true, &[]),
            chat("!orphan:example.org", MatrixRoomKind::Room, false, &[]),
        ];

        assert_eq!(
            derive_root_space_id_for_room("!dm:example.org", &chats),
            Some(VIRTUAL_DMS_ROOT_ID.to_string())
        );
        assert_eq!(
            derive_root_space_id_for_room("!orphan:example.org", &chats),
            Some(VIRTUAL_UNSPACED_ROOT_ID.to_string())
        );
    }

    #[test]
    fn builds_descendant_scoped_rooms_for_space() {
        let chats = vec![
            chat(
                "!space:example.org",
                MatrixRoomKind::Space,
                false,
                &["!child-space:example.org"],
            ),
            chat(
                "!child-space:example.org",
                MatrixRoomKind::Space,
                false,
                &["!child-room:example.org"],
            ),
            chat("!child-room:example.org", MatrixRoomKind::Room, false, &[]),
        ];

        let scoped = build_root_scoped_rooms(&chats, "!space:example.org");
        assert_eq!(scoped.len(), 2);
        assert!(scoped
            .iter()
            .any(|room| room.room_id == "!child-space:example.org"));
        assert!(scoped
            .iter()
            .any(|room| room.room_id == "!child-room:example.org"));
    }

    #[test]
    fn keeps_room_in_multiple_subspaces() {
        let chats = vec![
            chat(
                "!root:example.org",
                MatrixRoomKind::Space,
                false,
                &["!space-a:example.org", "!space-b:example.org"],
            ),
            chat(
                "!space-a:example.org",
                MatrixRoomKind::Space,
                false,
                &["!room:example.org"],
            ),
            chat(
                "!space-b:example.org",
                MatrixRoomKind::Space,
                false,
                &["!room:example.org"],
            ),
            chat("!room:example.org", MatrixRoomKind::Room, false, &[]),
        ];

        let scoped = build_root_scoped_rooms(&chats, "!root:example.org");
        let room_occurrences = scoped
            .iter()
            .filter(|room| room.room_id == "!room:example.org")
            .count();

        assert_eq!(room_occurrences, 2);
        assert!(scoped
            .iter()
            .any(|room| room.room_id == "!space-a:example.org"));
        assert!(scoped
            .iter()
            .any(|room| room.room_id == "!space-b:example.org"));
    }
}
