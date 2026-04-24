use std::collections::{HashMap, HashSet};

use matrix_sdk::ruma::api::client::space::get_hierarchy;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::rooms::types::{
    MatrixChatSummary, MatrixGetSpaceBrowseRequest, MatrixGetSpaceBrowseResponse, MatrixRoomKind,
};

use super::super::persistence::{collect_and_store_chats, load_cached_chats};

pub(super) async fn get_space_browse(
    request: MatrixGetSpaceBrowseRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetSpaceBrowseResponse, String> {
    let root_space_id = request.root_space_id.trim().to_string();
    if root_space_id.is_empty() {
        return Err(String::from("Root space id is required"));
    }

    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let mut joined_chats = collect_and_store_chats(&app_handle, &client).await;
    if joined_chats.is_empty() {
        joined_chats = load_cached_chats(&app_handle)?.unwrap_or_default();
    }

    let joined_by_id = joined_chats
        .into_iter()
        .map(|chat| (chat.room_id.clone(), chat))
        .collect::<HashMap<_, _>>();

    let hierarchy_rooms = load_space_hierarchy_rooms(&client, root_space_id.as_str()).await;
    let mut browse_rooms = hierarchy_rooms
        .into_iter()
        .map(|room| merge_with_joined_room(room, &joined_by_id))
        .collect::<Vec<_>>();

    if !browse_rooms.iter().any(|room| room.room_id == root_space_id) {
        if let Some(root_space) = joined_by_id.get(root_space_id.as_str()) {
            browse_rooms.insert(0, root_space.clone());
        }
    }

    // Keep deterministic order while preserving hierarchy relevance.
    browse_rooms.sort_by_cached_key(|room| room.display_name.to_lowercase());

    if let Some(index) = browse_rooms.iter().position(|room| room.room_id == root_space_id) {
        let root = browse_rooms.remove(index);
        browse_rooms.insert(0, root);
    }

    Ok(MatrixGetSpaceBrowseResponse {
        root_space_id,
        rooms: browse_rooms,
    })
}

async fn load_space_hierarchy_rooms(
    client: &matrix_sdk::Client,
    root_space_id: &str,
) -> Vec<MatrixChatSummary> {
    let Ok(root_room_id) = matrix_sdk::ruma::OwnedRoomId::try_from(root_space_id.to_owned()) else {
        return Vec::new();
    };

    let mut rooms_by_id = HashMap::<String, MatrixChatSummary>::new();
    let mut from = None::<String>;
    let mut seen_tokens = HashSet::<String>::new();
    let mut page_count = 0_usize;

    loop {
        let mut request = get_hierarchy::v1::Request::new(root_room_id.clone());
        request.from = from.clone();

        let response = match client.send(request).await {
            Ok(response) => response,
            Err(_) => break,
        };

        for chunk in response.rooms {
            let room_id = chunk.summary.room_id.to_string();
            let kind = if matches!(
                chunk.summary.room_type,
                Some(matrix_sdk::ruma::room::RoomType::Space)
            ) {
                MatrixRoomKind::Space
            } else {
                MatrixRoomKind::Room
            };

            let mut children_room_ids = Vec::new();
            for raw_child in chunk.children_state {
                let Ok(child) = raw_child.deserialize() else {
                    continue;
                };
                children_room_ids.push(child.state_key.to_string());
            }

            let display_name = chunk
                .summary
                .name
                .clone()
                .or_else(|| chunk.summary.canonical_alias.as_ref().map(ToString::to_string))
                .unwrap_or_else(|| room_id.clone());

            let room_summary = MatrixChatSummary {
                room_id: room_id.clone(),
                display_name,
                image_url: None,
                encrypted: chunk.summary.encryption.is_some(),
                joined_members: chunk.summary.num_joined_members.into(),
                kind,
                joined: false,
                is_direct: false,
                join_rule: Some(chunk.summary.join_rule.as_str().to_string()),
                world_readable: Some(chunk.summary.world_readable),
                guest_can_join: Some(chunk.summary.guest_can_join),
                children_room_ids,
            };

            rooms_by_id.insert(room_id, room_summary);
        }

        let Some(next_batch) = response.next_batch else {
            break;
        };

        if !seen_tokens.insert(next_batch.clone()) {
            break;
        }

        from = Some(next_batch);
        page_count += 1;

        // Guard against malformed servers looping tokens forever.
        if page_count >= 128 {
            break;
        }
    }

    rooms_by_id.into_values().collect()
}

fn merge_with_joined_room(
    mut candidate: MatrixChatSummary,
    joined_by_id: &HashMap<String, MatrixChatSummary>,
) -> MatrixChatSummary {
    let Some(joined) = joined_by_id.get(candidate.room_id.as_str()) else {
        return candidate;
    };

    candidate.image_url = joined.image_url.clone();
    candidate.joined = matches!(
        joined_by_id
            .get(candidate.room_id.as_str())
            .map(|room| room.joined),
        Some(true)
    );
    candidate.is_direct = joined.is_direct;
    candidate.encrypted = joined.encrypted || candidate.encrypted;
    candidate.join_rule = candidate.join_rule.clone().or(joined.join_rule.clone());
    candidate.world_readable = candidate.world_readable.or(joined.world_readable);
    candidate.guest_can_join = candidate.guest_can_join.or(joined.guest_can_join);

    if candidate.display_name == candidate.room_id {
        candidate.display_name = joined.display_name.clone();
    }

    if candidate.children_room_ids.is_empty() && !joined.children_room_ids.is_empty() {
        candidate.children_room_ids = joined.children_room_ids.clone();
    }

    // Preserve explicit local kind where available.
    candidate.kind = joined.kind.clone();

    candidate
}