use std::collections::{HashMap, HashSet};

use crate::rooms::types::{MatrixChatSummary, MatrixGetChatNavigationResponse, MatrixRoomKind};

pub(super) const VIRTUAL_DMS_ROOT_ID: &str = "virtual:dms";
pub(super) const VIRTUAL_UNSPACED_ROOT_ID: &str = "virtual:unspaced";

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

    fn matrix_root_spaces(&self) -> Vec<MatrixChatSummary> {
        self.chats
            .iter()
            .filter(|room| room.kind == MatrixRoomKind::Space)
            .filter(|space| self.parent_ids(space.room_id.as_str()).is_empty())
            .cloned()
            .collect()
    }

    fn build_root_spaces(&self, saved_root_space_ids: Option<&[String]>) -> Vec<MatrixChatSummary> {
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
            join_rule: None,
            world_readable: None,
            guest_can_join: None,
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
            join_rule: None,
            world_readable: None,
            guest_can_join: None,
            children_room_ids: vec![],
        };

        let matrix_root_spaces =
            order_matrix_root_spaces(self.matrix_root_spaces(), saved_root_space_ids);

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
        let mut discovered = HashSet::<&str>::new();
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

            if !discovered.insert(candidate.room_id.as_str()) {
                continue;
            }

            if candidate.kind == MatrixRoomKind::Space && candidate.joined_members == 0 {
                let mut next_ancestry = ancestry;
                next_ancestry.insert(room_id);

                if let Some(children) = self.children_by_parent.get(room_id) {
                    for child_id in children {
                        stack.push((child_id, next_ancestry.clone()));
                    }
                }

                continue;
            }

            if !candidate.joined {
                let mut next_ancestry = ancestry;
                next_ancestry.insert(room_id);

                if let Some(children) = self.children_by_parent.get(room_id) {
                    for child_id in children {
                        stack.push((child_id, next_ancestry.clone()));
                    }
                }

                continue;
            }

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

fn sort_rooms_by_display_name(rooms: &mut [MatrixChatSummary]) {
    rooms.sort_by_cached_key(|room| room.display_name.to_lowercase());
}

fn order_matrix_root_spaces(
    mut matrix_root_spaces: Vec<MatrixChatSummary>,
    saved_root_space_ids: Option<&[String]>,
) -> Vec<MatrixChatSummary> {
    sort_rooms_by_display_name(&mut matrix_root_spaces);

    let Some(saved_root_space_ids) = saved_root_space_ids else {
        return matrix_root_spaces;
    };

    if saved_root_space_ids.is_empty() {
        return matrix_root_spaces;
    }

    let mut room_by_id = matrix_root_spaces
        .into_iter()
        .map(|room| (room.room_id.clone(), room))
        .collect::<HashMap<_, _>>();

    let mut ordered_rooms = Vec::new();
    for room_id in saved_root_space_ids {
        if let Some(room) = room_by_id.remove(room_id) {
            ordered_rooms.push(room);
        }
    }

    let mut new_rooms = room_by_id.into_values().collect::<Vec<_>>();
    sort_rooms_by_display_name(&mut new_rooms);
    new_rooms.extend(ordered_rooms);
    new_rooms
}

pub(super) fn orderable_root_space_ids(chats: &[MatrixChatSummary]) -> HashSet<String> {
    NavigationIndex::new(chats)
        .matrix_root_spaces()
        .into_iter()
        .map(|room| room.room_id)
        .collect()
}

fn choose_selected_root_space_id(
    index: &NavigationIndex<'_>,
    requested_root_space_id: Option<&str>,
    selected_room_id: Option<&str>,
) -> Option<String> {
    if let Some(root_space_id) = requested_root_space_id {
        if index.is_root_space(root_space_id) {
            return Some(root_space_id.to_string());
        }
    }

    selected_room_id.and_then(|room_id| index.derive_root_space_id_for_room(room_id))
}

pub(super) fn build_navigation_response(
    chats: &[MatrixChatSummary],
    saved_root_space_ids: Option<&[String]>,
    requested_root_space_id: Option<&str>,
    selected_room_id: Option<&str>,
) -> MatrixGetChatNavigationResponse {
    let index = NavigationIndex::new(chats);

    let selected_root_space_id =
        choose_selected_root_space_id(&index, requested_root_space_id, selected_room_id);

    let root_spaces = index.build_root_spaces(saved_root_space_ids);
    let root_scoped_rooms = selected_root_space_id
        .as_deref()
        .map(|root_space_id| index.build_root_scoped_rooms(root_space_id))
        .unwrap_or_default();

    MatrixGetChatNavigationResponse {
        selected_root_space_id,
        root_spaces,
        root_scoped_rooms,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_navigation_response, orderable_root_space_ids, NavigationIndex, VIRTUAL_DMS_ROOT_ID,
        VIRTUAL_UNSPACED_ROOT_ID,
    };
    use crate::rooms::types::{MatrixChatSummary, MatrixRoomKind};

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
            join_rule: None,
            world_readable: None,
            guest_can_join: None,
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

        let response = build_navigation_response(&chats, None, None, None);
        let roots = response.root_spaces;

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

        let index = NavigationIndex::new(&chats);

        assert_eq!(
            index.derive_root_space_id_for_room("!dm:example.org"),
            Some(VIRTUAL_DMS_ROOT_ID.to_string())
        );
        assert_eq!(
            index.derive_root_space_id_for_room("!orphan:example.org"),
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

        let response = build_navigation_response(
            &chats,
            None,
            Some("!space:example.org"),
            Some("!child-room:example.org"),
        );
        let scoped = response.root_scoped_rooms;

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

        let response = build_navigation_response(
            &chats,
            None,
            Some("!root:example.org"),
            Some("!room:example.org"),
        );
        let scoped = response.root_scoped_rooms;

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

    #[test]
    fn reorders_root_spaces_with_saved_order_and_new_rooms_first() {
        let chats = vec![
            chat("!alpha:example.org", MatrixRoomKind::Space, false, &[]),
            chat("!beta:example.org", MatrixRoomKind::Space, false, &[]),
            chat("!gamma:example.org", MatrixRoomKind::Space, false, &[]),
        ];

        let saved_root_space_ids = vec![
            "!beta:example.org".to_string(),
            "!alpha:example.org".to_string(),
        ];
        let response =
            build_navigation_response(&chats, Some(saved_root_space_ids.as_slice()), None, None);

        assert_eq!(response.root_spaces[2].room_id, "!gamma:example.org");
        assert_eq!(response.root_spaces[3].room_id, "!beta:example.org");
        assert_eq!(response.root_spaces[4].room_id, "!alpha:example.org");
    }

    #[test]
    fn validates_orderable_root_space_ids() {
        let chats = vec![
            chat("!root:example.org", MatrixRoomKind::Space, false, &[]),
            chat("!child:example.org", MatrixRoomKind::Room, false, &[]),
        ];

        let orderable = orderable_root_space_ids(&chats);

        assert!(orderable.contains("!root:example.org"));
        assert!(!orderable.contains("!child:example.org"));
    }
}
