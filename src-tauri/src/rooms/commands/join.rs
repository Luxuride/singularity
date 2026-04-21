use matrix_sdk::ruma::{OwnedServerName, RoomOrAliasId};
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

    let server_names: Vec<OwnedServerName> = merged_server_names
        .iter()
        .filter_map(|s| {
            <&matrix_sdk::ruma::ServerName>::try_from(s.as_str())
                .ok()
                .map(|name| name.to_owned())
        })
        .collect();

    let response = client
        .join_room_by_id_or_alias(room_id_or_alias, &server_names)
        .await
        .map_err(|e| format!("Failed to join room: {e}"))?;

    Ok(MatrixJoinRoomResponse {
        room_id: response.room_id().to_string(),
    })
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
    use super::{merge_server_name_candidates, parse_join_link_input};

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
}
