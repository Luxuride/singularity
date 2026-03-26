use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::protocol::config;
use crate::protocol::sync::sync_once_serialized;

use super::persistence::{load_cached_chats, store_cached_chats};
use super::types::{
    MatrixChatSummary, MatrixGetChatNavigationRequest, MatrixGetChatNavigationResponse,
    MatrixGetChatsResponse, MatrixRoomKind,
};
use super::workers::collect_chat_summaries;
use super::{
    MatrixTriggerRoomUpdateRequest, MatrixTriggerRoomUpdateResponse, RoomRefreshTrigger,
    RoomUpdateTriggerState,
};

const VIRTUAL_DMS_ROOT_ID: &str = "virtual:dms";
const VIRTUAL_UNSPACED_ROOT_ID: &str = "virtual:unspaced";

struct NavigationIndex<'a> {
    chats: &'a [MatrixChatSummary],
    room_by_id: HashMap<&'a str, &'a MatrixChatSummary>,
    valid_parent_ids_by_room: HashMap<&'a str, Vec<&'a str>>,
    children_by_parent: HashMap<&'a str, Vec<&'a str>>,
}

impl<'a> NavigationIndex<'a> {
    fn new(chats: &'a [MatrixChatSummary]) -> Self {
        let room_by_id = chats
            .iter()
            .map(|room| (room.room_id.as_str(), room))
            .collect::<HashMap<_, _>>();

        let mut valid_parent_ids_by_room = HashMap::<&str, Vec<&str>>::new();
        let mut children_by_parent = HashMap::<&str, Vec<&str>>::new();

        for room in chats {
            let mut parents = Vec::<&str>::new();

            if let Some(parent_room_id) = room.parent_room_id.as_deref() {
                parents.push(parent_room_id);
            }

            for parent_room_id in &room.parent_room_ids {
                let parent_room_id = parent_room_id.as_str();
                if !parents.contains(&parent_room_id) {
                    parents.push(parent_room_id);
                }
            }

            let mut valid_parents = Vec::<&str>::new();
            for parent_id in parents {
                if parent_id == room.room_id {
                    continue;
                }

                if !room_by_id.contains_key(parent_id) {
                    continue;
                }

                valid_parents.push(parent_id);
            }

            for parent_id in &valid_parents {
                children_by_parent
                    .entry(parent_id)
                    .or_default()
                    .push(room.room_id.as_str());
            }

            valid_parent_ids_by_room.insert(room.room_id.as_str(), valid_parents);
        }

        Self {
            chats,
            room_by_id,
            valid_parent_ids_by_room,
            children_by_parent,
        }
    }

    fn chat(&self, room_id: &str) -> Option<&MatrixChatSummary> {
        self.room_by_id.get(room_id).copied()
    }

    fn valid_parent_ids(&self, room_id: &str) -> &[&str] {
        self.valid_parent_ids_by_room
            .get(room_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn is_unspaced_room(&self, room: &MatrixChatSummary) -> bool {
        if room.kind != MatrixRoomKind::Room || room.is_direct {
            return false;
        }

        self.valid_parent_ids(room.room_id.as_str()).is_empty()
    }

    fn primary_parent_id(&self, room: &MatrixChatSummary) -> Option<String> {
        self.valid_parent_ids(room.room_id.as_str())
            .first()
            .map(|value| (*value).to_string())
    }

    fn root_parent_space_id(&self, room: &MatrixChatSummary) -> Option<String> {
        let mut stack = self.valid_parent_ids(room.room_id.as_str()).to_vec();
        let mut seen = HashSet::<&str>::new();

        while let Some(parent_id) = stack.pop() {
            if !seen.insert(parent_id) {
                continue;
            }

            let Some(parent_room) = self.chat(parent_id) else {
                continue;
            };

            let parent_parents = self.valid_parent_ids(parent_room.room_id.as_str());
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

        room.kind == MatrixRoomKind::Space
            && self.valid_parent_ids(room.room_id.as_str()).is_empty()
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
                && self.valid_parent_ids(room.room_id.as_str()).is_empty()
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
            parent_room_id: None,
            parent_room_ids: vec![],
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
            parent_room_id: None,
            parent_room_ids: vec![],
        };

        let mut matrix_root_spaces = self
            .chats
            .iter()
            .filter(|room| room.kind == MatrixRoomKind::Space)
            .filter(|space| self.valid_parent_ids(space.room_id.as_str()).is_empty())
            .cloned()
            .collect::<Vec<_>>();
        sort_rooms_by_display_name(&mut matrix_root_spaces);

        let mut roots = Vec::with_capacity(matrix_root_spaces.len() + 2);
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
                .map(|room| {
                    let mut room = room.clone();
                    room.parent_room_id = self.primary_parent_id(&room);
                    room
                })
                .collect::<Vec<_>>();
            sort_rooms_by_display_name(&mut rooms);
            return rooms;
        }

        let mut descendants = Vec::<MatrixChatSummary>::new();
        let mut stack: Vec<(&str, &str, HashSet<&str>)> = self
            .children_by_parent
            .get(root_space_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|child_id| (child_id, root_space_id, HashSet::new()))
            .collect();

        while let Some((room_id, parent_id, ancestry)) = stack.pop() {
            if ancestry.contains(room_id) {
                continue;
            }

            let Some(candidate) = self.chat(room_id) else {
                continue;
            };

            let mut candidate = candidate.clone();
            candidate.parent_room_id = Some(parent_id.to_string());
            descendants.push(candidate);

            let mut next_ancestry = ancestry;
            next_ancestry.insert(room_id);

            if let Some(children) = self.children_by_parent.get(room_id) {
                for child_id in children {
                    stack.push((child_id, room_id, next_ancestry.clone()));
                }
            }
        }

        sort_rooms_by_display_name(&mut descendants);
        descendants
    }
}

fn has_stale_in_memory_chat_media(chats: &MatrixGetChatsResponse) -> bool {
    chats.chats.iter().any(|chat| {
        chat.image_url
            .as_deref()
            .is_some_and(|url| url.starts_with("matrix-media://"))
    })
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

        if has_stale_in_memory_chat_media(&cached) {
            let client = auth_state.restore_client_and_get(&app_handle).await?;

            let local_chats = collect_chat_summaries(&client).await;
            if !local_chats.is_empty() {
                let _ = store_cached_chats(&app_handle, &local_chats);
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

    let local_chats = collect_chat_summaries(&client).await;
    if !local_chats.is_empty() {
        let _ = store_cached_chats(&app_handle, &local_chats);
        let _ = trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: None,
            include_selected_messages: false,
        });

        return Ok(MatrixGetChatsResponse { chats: local_chats });
    }

    sync_once_serialized(
        &client,
        matrix_sdk::config::SyncSettings::default()
            .timeout(Duration::from_secs(config::SYNC_TIMEOUT_SECONDS)),
    )
    .await
    .map_err(|error| format!("Failed to sync Matrix rooms: {error}"))?;

    let chats = collect_chat_summaries(&client).await;

    store_cached_chats(&app_handle, &chats)?;

    let _ = trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: None,
        include_selected_messages: false,
    });

    Ok(MatrixGetChatsResponse { chats })
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
        has_stale_in_memory_chat_media, VIRTUAL_DMS_ROOT_ID, VIRTUAL_UNSPACED_ROOT_ID,
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
            parent_room_id: None,
            parent_room_ids: vec![],
        }
    }

    #[test]
    fn detects_stale_matrix_media_avatar_url() {
        let response = MatrixGetChatsResponse {
            chats: vec![chat_with_image(Some(
                "matrix-media://localhost/img-123.png",
            ))],
        };

        assert!(has_stale_in_memory_chat_media(&response));
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

        assert!(!has_stale_in_memory_chat_media(&response));
    }

    fn chat(
        room_id: &str,
        kind: MatrixRoomKind,
        is_direct: bool,
        parent_room_id: Option<&str>,
    ) -> MatrixChatSummary {
        let parent_room_ids = parent_room_id
            .map(|value| vec![value.to_string()])
            .unwrap_or_default();

        MatrixChatSummary {
            room_id: room_id.to_string(),
            display_name: room_id.to_string(),
            image_url: None,
            encrypted: false,
            joined_members: 0,
            kind,
            joined: true,
            is_direct,
            parent_room_id: parent_room_id.map(ToOwned::to_owned),
            parent_room_ids,
        }
    }

    #[test]
    fn builds_virtual_roots_with_counts() {
        let chats = vec![
            chat("!dm:example.org", MatrixRoomKind::Room, true, None),
            chat("!orphan:example.org", MatrixRoomKind::Room, false, None),
        ];

        let roots = build_root_spaces(&chats);
        assert_eq!(roots[0].room_id, VIRTUAL_DMS_ROOT_ID);
        assert_eq!(roots[0].joined_members, 1);
        assert_eq!(roots[1].room_id, VIRTUAL_UNSPACED_ROOT_ID);
        assert_eq!(roots[1].joined_members, 1);
    }

    #[test]
    fn derives_virtual_root_for_direct_and_unspaced_room() {
        let chats = vec![
            chat("!dm:example.org", MatrixRoomKind::Room, true, None),
            chat("!orphan:example.org", MatrixRoomKind::Room, false, None),
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
            chat("!space:example.org", MatrixRoomKind::Space, false, None),
            chat(
                "!child-space:example.org",
                MatrixRoomKind::Space,
                false,
                Some("!space:example.org"),
            ),
            chat(
                "!child-room:example.org",
                MatrixRoomKind::Room,
                false,
                Some("!child-space:example.org"),
            ),
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
        let mut multi_parent_room = chat("!room:example.org", MatrixRoomKind::Room, false, None);
        multi_parent_room.parent_room_ids = vec![
            String::from("!space-a:example.org"),
            String::from("!space-b:example.org"),
        ];

        let chats = vec![
            chat("!root:example.org", MatrixRoomKind::Space, false, None),
            chat(
                "!space-a:example.org",
                MatrixRoomKind::Space,
                false,
                Some("!root:example.org"),
            ),
            chat(
                "!space-b:example.org",
                MatrixRoomKind::Space,
                false,
                Some("!root:example.org"),
            ),
            multi_parent_room,
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
