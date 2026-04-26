use matrix_sdk::ruma::api::client::space::get_hierarchy;
use matrix_sdk::ruma::{OwnedRoomId, OwnedServerName, RoomId, RoomOrAliasId};
use percent_encoding::percent_decode_str;
use std::convert::TryFrom;
use tauri::{AppHandle, State};
use url::Url;

use crate::auth::AuthState;
use crate::rooms::types::{MatrixJoinRoomRequest, MatrixJoinRoomResponse};

pub async fn join_room(
    request: MatrixJoinRoomRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixJoinRoomResponse, String> {
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let (room_id_or_alias_raw, via_server_names) = parse_join_link_input(&request.room_id_or_alias);
    let merged_server_names = merge_server_name_candidates(request.server_names, via_server_names);

    let room_id_or_alias = <&RoomOrAliasId>::try_from(room_id_or_alias_raw.as_str())
        .map_err(|e| format!("Invalid room ID or alias: {e}"))?;

    let mut merged_server_names = ensure_room_target_server_name(merged_server_names, room_id_or_alias);
    let mut server_names = to_owned_server_names(&merged_server_names);

    let response = match client
        .join_room_by_id_or_alias(room_id_or_alias, &server_names)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let error_message = error.to_string();

            if !is_missing_via_join_error(error_message.as_str()) {
                return Err(format!("Failed to join room: {error}"));
            }

            let discovered_via_server_names = discover_via_server_names_from_joined_spaces(
                &client,
                room_id_or_alias,
            )
            .await;
            let discovered_via_debug = discovered_via_server_names.join(",");

            let retry_server_name_candidates = with_fallback_server_names(
                room_id_or_alias,
                merged_server_names,
                discovered_via_server_names,
            );

            if retry_server_name_candidates.is_empty() {
                return Err(format!("Failed to join room: {error}"));
            }

            server_names = to_owned_server_names(&retry_server_name_candidates);
            if server_names.is_empty() {
                return Err(format!("Failed to join room: {error}"));
            }

            merged_server_names = retry_server_name_candidates;

            match client
                .join_room_by_id_or_alias(room_id_or_alias, &server_names)
                .await
            {
                Ok(response) => response,
                Err(retry_error) => {
                    if target_server_matches_homeserver(&client, room_id_or_alias) {
                        if let Some(room_id) = parse_room_id(room_id_or_alias) {
                            if let Ok(response) = client.join_room_by_id(room_id).await {
                                return Ok(MatrixJoinRoomResponse {
                                    room_id: response.room_id().to_string(),
                                });
                            }
                        }
                    }

                    let alias_candidates = discover_room_alias_candidates_from_joined_spaces(
                        &client,
                        room_id_or_alias,
                    )
                    .await;

                    for alias in &alias_candidates {
                        let Ok(alias_id_or_room) = <&RoomOrAliasId>::try_from(alias.as_str()) else {
                            continue;
                        };

                        let alias_server_names = ensure_room_target_server_name(
                            merged_server_names.clone(),
                            alias_id_or_room,
                        );
                        let alias_server_names = to_owned_server_names(&alias_server_names);
                        if alias_server_names.is_empty() {
                            continue;
                        }

                        let joined = client
                            .join_room_by_id_or_alias(alias_id_or_room, &alias_server_names)
                            .await;

                        if let Ok(response) = joined {
                            return Ok(MatrixJoinRoomResponse {
                                room_id: response.room_id().to_string(),
                            });
                        }
                    }

                    let alias_candidates_debug = alias_candidates.join(",");
                    return Err(format!(
                        "Failed to join room: {retry_error} (initial error: {error_message}; discovered hierarchy via: [{}]; retry via candidates: [{}]; alias candidates: [{}])",
                        discovered_via_debug,
                        merged_server_names.join(","),
                        alias_candidates_debug,
                    ));
                }
            }
        }
    };

    Ok(MatrixJoinRoomResponse {
        room_id: response.room_id().to_string(),
    })
}

fn to_owned_server_names(server_names: &[String]) -> Vec<OwnedServerName> {
    server_names
        .iter()
        .filter_map(|candidate| {
            <&matrix_sdk::ruma::ServerName>::try_from(candidate.as_str())
                .ok()
                .map(|name| name.to_owned())
        })
        .collect()
}

fn is_missing_via_join_error(error_message: &str) -> bool {
    error_message
        .to_ascii_lowercase()
        .contains("no servers that are in the room have been provided")
}

fn with_fallback_server_names(
    room_id_or_alias: &RoomOrAliasId,
    mut server_names: Vec<String>,
    discovered_via_server_names: Vec<String>,
) -> Vec<String> {
    server_names.extend(discovered_via_server_names);

    if let Some(server_name) = room_id_or_alias.server_name() {
        server_names.push(server_name.to_string());
    }

    dedupe_preserve_order(server_names)
}

async fn discover_via_server_names_from_joined_spaces(
    client: &matrix_sdk::Client,
    room_id_or_alias: &RoomOrAliasId,
) -> Vec<String> {
    let target_room_id = room_id_or_alias.as_str().trim().to_string();
    if !target_room_id.starts_with('!') {
        return Vec::new();
    }

    let mut via_server_names = Vec::<String>::new();
    let joined_space_ids = client
        .joined_rooms()
        .into_iter()
        .filter(|room| room.is_space())
        .filter_map(|room| OwnedRoomId::try_from(room.room_id().to_string()).ok())
        .collect::<Vec<_>>();

    for space_id in joined_space_ids {
        if let Some(space_server_name) = space_id.server_name() {
            via_server_names.push(space_server_name.to_string());
        }

        let mut from = None::<String>;
        let mut seen_tokens = std::collections::HashSet::<String>::new();
        let mut page_count = 0_usize;

        loop {
            let mut request = get_hierarchy::v1::Request::new(space_id.clone());
            request.from = from.clone();

            let Ok(response) = client.send(request).await else {
                break;
            };

            for chunk in response.rooms {
                for raw_child in chunk.children_state {
                    let Ok(child) = raw_child.deserialize_as::<serde_json::Value>() else {
                        continue;
                    };

                    let Some(child_room_id) = child
                        .get("state_key")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    else {
                        continue;
                    };

                    if child_room_id != target_room_id {
                        continue;
                    }

                    if let Some(parent_server_name) = chunk.summary.room_id.server_name() {
                        via_server_names.push(parent_server_name.to_string());
                    }

                    let Some(via_values) = child
                        .get("content")
                        .and_then(|content| content.get("via"))
                        .and_then(|via| via.as_array())
                    else {
                        continue;
                    };

                    for via_value in via_values {
                        let Some(server_name) = via_value.as_str().map(str::trim) else {
                            continue;
                        };

                        if server_name.is_empty()
                            || via_server_names
                                .iter()
                                .any(|existing| existing == server_name)
                        {
                            continue;
                        }

                        via_server_names.push(server_name.to_string());
                    }
                }
            }

            let Some(next_batch) = response.next_batch else {
                break;
            };

            if !seen_tokens.insert(next_batch.clone()) {
                break;
            }

            from = Some(next_batch);
            page_count += 1;

            if page_count >= 64 {
                break;
            }
        }
    }

    dedupe_preserve_order(via_server_names)
}

fn target_server_matches_homeserver(
    client: &matrix_sdk::Client,
    room_id_or_alias: &RoomOrAliasId,
) -> bool {
    let Some(target_server_name) = room_id_or_alias.server_name() else {
        return false;
    };

    let target_server = target_server_name.to_string();
    let homeserver = client.homeserver();
    let Some(homeserver_host) = homeserver.host_str() else {
        return false;
    };

    if target_server == homeserver_host {
        return true;
    }

    if let Some(homeserver_port) = homeserver.port() {
        return target_server == format!("{homeserver_host}:{homeserver_port}");
    }

    false
}

fn parse_room_id<'a>(room_id_or_alias: &'a RoomOrAliasId) -> Option<&'a RoomId> {
    <&RoomId>::try_from(room_id_or_alias.as_str()).ok()
}

async fn discover_room_alias_candidates_from_joined_spaces(
    client: &matrix_sdk::Client,
    room_id_or_alias: &RoomOrAliasId,
) -> Vec<String> {
    let target_room_id = room_id_or_alias.as_str().trim().to_string();
    if !target_room_id.starts_with('!') {
        return Vec::new();
    }

    let joined_space_ids = client
        .joined_rooms()
        .into_iter()
        .filter(|room| room.is_space())
        .filter_map(|room| OwnedRoomId::try_from(room.room_id().to_string()).ok())
        .collect::<Vec<_>>();

    let mut alias_candidates = Vec::<String>::new();

    for space_id in joined_space_ids {
        let mut from = None::<String>;
        let mut seen_tokens = std::collections::HashSet::<String>::new();
        let mut page_count = 0_usize;

        loop {
            let mut request = get_hierarchy::v1::Request::new(space_id.clone());
            request.from = from.clone();

            let Ok(response) = client.send(request).await else {
                break;
            };

            for chunk in &response.rooms {
                if chunk.summary.room_id.to_string() != target_room_id {
                    continue;
                }

                if let Some(canonical_alias) = chunk.summary.canonical_alias.as_ref() {
                    let alias = canonical_alias.to_string();
                    if alias.starts_with('#') && !alias_candidates.iter().any(|existing| existing == &alias) {
                        alias_candidates.push(alias);
                    }
                }
            }

            let Some(next_batch) = response.next_batch else {
                break;
            };

            if !seen_tokens.insert(next_batch.clone()) {
                break;
            }

            from = Some(next_batch);
            page_count += 1;
            if page_count >= 64 {
                break;
            }
        }
    }

    alias_candidates
}

fn parse_join_link_input(input: &str) -> (String, Vec<String>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (String::new(), Vec::new());
    }

    if let Ok(parsed) = Url::parse(trimmed) {
        let mut via_server_names = parse_via_query(parsed.query());

        let mut target = String::new();
        if let Some(fragment) = parsed.fragment() {
            let (fragment_target, fragment_query) = split_target_and_query(fragment);
            via_server_names.extend(parse_via_query(fragment_query));

            target = decode_once(fragment_target.trim_start_matches('/'));
            target = normalize_target_for_scheme(&parsed, target);
        }

        if target.is_empty() {
            let path = parsed.path().trim_start_matches('/');
            target = decode_once(path);
            target = normalize_target_for_scheme(&parsed, target);
        }

        if target.is_empty() {
            target = trimmed.to_owned();
        }

        return (target, dedupe_preserve_order(via_server_names));
    }

    let (raw_target, query) = split_target_and_query(trimmed);
    let target = decode_once(raw_target);
    let via_server_names = dedupe_preserve_order(parse_via_query(query));
    (target, via_server_names)
}

fn split_target_and_query(value: &str) -> (&str, Option<&str>) {
    match value.split_once('?') {
        Some((target, query)) => (target, Some(query)),
        None => (value, None),
    }
}

fn parse_via_query(query: Option<&str>) -> Vec<String> {
    let mut via_values = Vec::new();
    if let Some(raw_query) = query {
        for (key, value) in url::form_urlencoded::parse(raw_query.as_bytes()) {
            if key == "via" {
                let candidate = value.trim();
                if !candidate.is_empty() {
                    via_values.push(candidate.to_owned());
                }
            }
        }
    }

    via_values
}

fn decode_once(value: &str) -> String {
    percent_decode_str(value)
        .decode_utf8_lossy()
        .trim()
        .to_owned()
}

fn normalize_target_for_scheme(parsed: &Url, target: String) -> String {
    if target.is_empty() {
        return target;
    }

    if target.starts_with('#') || target.starts_with('!') {
        return target;
    }

    if parsed.scheme() == "matrix" {
        if let Some(rest) = target.strip_prefix("r/") {
            return format!("#{rest}");
        }
        if let Some(rest) = target.strip_prefix("room/") {
            return format!("#{rest}");
        }
        if let Some(rest) = target.strip_prefix("roomid/") {
            return rest.to_owned();
        }
        if let Some(rest) = target.strip_prefix("s/") {
            return format!("#{rest}");
        }
        if let Some(rest) = target.strip_prefix("space/") {
            return format!("#{rest}");
        }
    }

    if parsed.scheme() == "singularity" && target.contains(':') {
        return format!("#{target}");
    }

    target
}

fn merge_server_name_candidates(
    primary: Option<Vec<String>>,
    secondary: Vec<String>,
) -> Vec<String> {
    let mut merged = primary.unwrap_or_default();
    merged.extend(secondary);
    dedupe_preserve_order(merged)
}

fn ensure_room_target_server_name(
    mut server_names: Vec<String>,
    room_id_or_alias: &RoomOrAliasId,
) -> Vec<String> {
    if !server_names.is_empty() {
        return server_names;
    }

    if let Some(server_name) = room_id_or_alias.server_name() {
        server_names.push(server_name.to_string());
    }

    server_names
}

fn dedupe_preserve_order(values: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for value in values {
        if !deduped.iter().any(|existing| existing == &value) {
            deduped.push(value);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::{
        is_missing_via_join_error, merge_server_name_candidates, parse_join_link_input,
    };

    #[test]
    fn parses_matrix_to_fragment_via_values() {
        let (target, via) = parse_join_link_input(
            "https://matrix.to/#/%23room:matrix.org?via=matrix.org&via=example.com",
        );

        assert_eq!(target, "#room:matrix.org");
        assert_eq!(via, vec!["matrix.org", "example.com"]);
    }

    #[test]
    fn parses_singularity_join_link_and_adds_alias_prefix() {
        let (target, via) = parse_join_link_input(
            "singularity://join/#example:matrix.org?via=matrix.org&via=matrix.org",
        );

        assert_eq!(target, "#example:matrix.org");
        assert_eq!(via, vec!["matrix.org"]);
    }

    #[test]
    fn parses_raw_alias_with_query_via_values() {
        let (target, via) = parse_join_link_input("#room:matrix.org?via=matrix.org");

        assert_eq!(target, "#room:matrix.org");
        assert_eq!(via, vec!["matrix.org"]);
    }

    #[test]
    fn parses_multiple_via_from_query_and_fragment_query() {
        let (target, via) = parse_join_link_input(
            "https://matrix.to/#/%23room:matrix.org?via=matrix-a.org&via=matrix-b.org&via=matrix-c.org",
        );

        assert_eq!(target, "#room:matrix.org");
        assert_eq!(via, vec!["matrix-a.org", "matrix-b.org", "matrix-c.org"]);
    }

    #[test]
    fn merges_explicit_and_parsed_via_values() {
        let merged = merge_server_name_candidates(
            Some(vec!["client-a.org".to_owned(), "client-b.org".to_owned()]),
            vec![
                "matrix-a.org".to_owned(),
                "client-b.org".to_owned(),
                "matrix-c.org".to_owned(),
            ],
        );

        assert_eq!(
            merged,
            vec![
                "client-a.org",
                "client-b.org",
                "matrix-a.org",
                "matrix-c.org",
            ]
        );
    }

    #[test]
    fn identifies_missing_via_join_error_message() {
        let error_message = "the server returned an error: [404 / M_UNKNOWN] Can't join remote room because no servers that are in the room have been provided.";

        assert!(is_missing_via_join_error(error_message));
    }

}
