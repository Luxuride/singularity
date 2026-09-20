//! Shared parsing of the `m.direct` account-data event.

use std::collections::{HashMap, HashSet};

use matrix_sdk::ruma::events::GlobalAccountDataEventType;

/// Load the `m.direct` mapping: user ID -> list of direct room IDs. Returns an
/// empty map when the account data is absent or malformed.
pub async fn direct_room_mapping(
    client: &matrix_sdk::Client,
) -> HashMap<String, Vec<String>> {
    let mut mapping = HashMap::<String, Vec<String>>::new();

    let raw_content = match client
        .account()
        .account_data_raw(GlobalAccountDataEventType::from("m.direct"))
        .await
    {
        Ok(raw_content) => raw_content,
        Err(_) => return mapping,
    };

    let Some(raw_content) = raw_content else {
        return mapping;
    };

    let Ok(content) = raw_content.deserialize_as::<serde_json::Value>() else {
        return mapping;
    };

    let Some(content) = content.as_object() else {
        return mapping;
    };

    for (user_id, room_ids) in content {
        let Some(room_ids) = room_ids.as_array() else {
            continue;
        };

        let mut ids = Vec::new();
        for room_id in room_ids {
            let Some(room_id) = room_id.as_str() else {
                continue;
            };
            if !room_id.is_empty() {
                ids.push(room_id.to_string());
            }
        }

        if !ids.is_empty() {
            mapping.insert(user_id.to_string(), ids);
        }
    }

    mapping
}

/// The set of all room IDs that appear in any `m.direct` entry.
pub async fn direct_room_ids(client: &matrix_sdk::Client) -> HashSet<String> {
    let mut ids = HashSet::new();
    for room_ids in direct_room_mapping(client).await.values() {
        for room_id in room_ids {
            ids.insert(room_id.to_string());
        }
    }
    ids
}

/// The other participant's user ID for a direct room, or None when the room is
/// not in `m.direct` or only maps to the current user.
pub async fn direct_room_target_user_id(
    client: &matrix_sdk::Client,
    room_id: &str,
) -> Option<String> {
    let own_user_id = client.user_id().map(|value| value.as_str().to_string());
    for (user_id, room_ids) in direct_room_mapping(client).await {
        if own_user_id.as_deref() == Some(user_id.as_str()) {
            continue;
        }

        if room_ids.iter().any(|candidate| candidate == room_id) {
            return Some(user_id.to_string());
        }
    }

    None
}