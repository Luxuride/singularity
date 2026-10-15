//! Per-room notification keywords, implemented as custom override push
//! rules that match both the room and a `content.body` pattern. The
//! matrix-sdk keyword helpers are global-only, so the rules are managed
//! through the SDK's custom conditional-rule API; rule IDs are
//! `singularity.room_keyword.<room_id>.<hex(keyword)>` so they can be
//! recognised and removed individually.

use std::sync::Arc;

use auth::state::AuthState;
use matrix_sdk::notification_settings::NotificationSettings;
use matrix_sdk::ruma::api::client::push::delete_pushrule;
use matrix_sdk::ruma::push::{
    Action, EventMatchConditionData, PushCondition, RuleKind, Ruleset, SoundTweakValue, Tweak,
};
use matrix_sdk::ruma::RoomId;
use matrix_sdk::Client;
use storage::AppDb;
use types::settings::{
    MatrixAddRoomKeywordRequest, MatrixAddRoomKeywordResponse, MatrixGetRoomKeywordsRequest,
    MatrixRemoveRoomKeywordRequest, MatrixRemoveRoomKeywordResponse, MatrixRoomKeywordsResponse,
    ROOM_KEYWORD_RULE_PREFIX,
};
use types::Paths;

use crate::commands::require_client;

const KEYWORD_MAX_LEN: usize = 64;

pub(crate) fn validate_keyword(keyword: &str) -> Result<(), String> {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return Err("Keyword cannot be empty".to_string());
    }
    if trimmed.len() > KEYWORD_MAX_LEN {
        return Err(format!(
            "Keyword is too long (maximum {KEYWORD_MAX_LEN} characters)"
        ));
    }
    Ok(())
}

pub(crate) fn rule_id_for(room_id: &RoomId, keyword: &str) -> String {
    format!(
        "{ROOM_KEYWORD_RULE_PREFIX}{room_id}.{}",
        hex_encode(keyword)
    )
}

fn hex_encode(value: &str) -> String {
    value.bytes().map(|byte| format!("{byte:02x}")).collect()
}

fn keyword_from_rule_id(room_id: &RoomId, rule_id: &str) -> Option<String> {
    let encoded = rule_id
        .strip_prefix(ROOM_KEYWORD_RULE_PREFIX)?
        .strip_prefix(room_id.as_str())?
        .strip_prefix('.')?;

    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks(2) {
        if pair.len() != 2 {
            return None;
        }
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        bytes.push(((high << 4) | low) as u8);
    }

    String::from_utf8(bytes).ok()
}

/// Keyword rules are matched under both `Override` (this module) and, for
/// defence in depth, `Underride` — a ruleset may carry duplicates if the
/// rule-ID scheme ever changes.
fn room_keyword_rule_ids(ruleset: &Ruleset, room_id: &RoomId) -> Vec<(RuleKind, String)> {
    let matches = |rule_id: &str| {
        rule_id.starts_with(ROOM_KEYWORD_RULE_PREFIX)
            && keyword_from_rule_id(room_id, rule_id).is_some()
    };

    let mut ids: Vec<(RuleKind, String)> = ruleset
        .override_
        .iter()
        .filter(|rule| matches(&rule.rule_id))
        .map(|rule| (RuleKind::Override, rule.rule_id.clone()))
        .collect();
    ids.extend(
        ruleset
            .underride
            .iter()
            .filter(|rule| matches(&rule.rule_id))
            .map(|rule| (RuleKind::Underride, rule.rule_id.clone())),
    );
    ids
}

/// Collect the room's own keyword rules and decode the keywords from their
/// rule IDs. Non-keyword custom rules for the room are ignored.
pub(crate) fn room_keywords_from_ruleset(ruleset: &Ruleset, room_id: &RoomId) -> Vec<String> {
    let mut keywords: Vec<String> = room_keyword_rule_ids(ruleset, room_id)
        .into_iter()
        .filter_map(|(kind, rule_id)| match kind {
            RuleKind::Override | RuleKind::Underride => keyword_from_rule_id(room_id, &rule_id),
            _ => None,
        })
        .collect();
    keywords.sort();
    keywords.dedup();
    keywords
}

async fn room_keywords(
    settings: &NotificationSettings,
    room_id: &RoomId,
) -> Result<Vec<String>, String> {
    let ruleset = settings.ruleset().await;
    Ok(room_keywords_from_ruleset(&ruleset, room_id))
}

fn keyword_actions() -> Vec<Action> {
    vec![
        Action::Notify,
        Action::SetTweak(Tweak::Sound(SoundTweakValue::Default)),
    ]
}

fn keyword_conditions(room_id: &RoomId, keyword: &str) -> Vec<PushCondition> {
    vec![
        PushCondition::EventMatch(EventMatchConditionData::new(
            String::from("room_id"),
            room_id.to_string(),
        )),
        PushCondition::EventMatch(EventMatchConditionData::new(
            String::from("content.body"),
            keyword.to_owned(),
        )),
    ]
}

async fn create_room_keyword_rule(
    settings: &NotificationSettings,
    room_id: &RoomId,
    keyword: &str,
) -> Result<(), String> {
    settings
        .create_custom_conditional_push_rule(
            rule_id_for(room_id, keyword),
            RuleKind::Override,
            keyword_actions(),
            keyword_conditions(room_id, keyword),
        )
        .await
        .map_err(|error| format!("Failed to add room keyword: {error}"))
}

/// Re-create the room's keyword rules. Needed after the notification mode
/// changed: setting a mode deletes every other custom rule for the room.
pub(crate) async fn recreate_room_keywords(
    settings: &NotificationSettings,
    room_id: &RoomId,
    keywords: &[String],
) -> Result<(), String> {
    for keyword in keywords {
        create_room_keyword_rule(settings, room_id, keyword).await?;
    }
    Ok(())
}

/// Delete the room's keyword rules without touching its other custom
/// rules, so that changing the notification mode can keep them.
pub(crate) async fn delete_room_keyword_rules(
    settings: &NotificationSettings,
    client: &Client,
    room_id: &RoomId,
) -> Result<(), String> {
    let ruleset = settings.ruleset().await;
    for (kind, rule_id) in room_keyword_rule_ids(&ruleset, room_id) {
        delete_rule(client, &kind, &rule_id).await?;
    }
    Ok(())
}

async fn delete_rule(client: &Client, kind: &RuleKind, rule_id: &str) -> Result<(), String> {
    let request = delete_pushrule::v3::Request::new(kind.clone(), rule_id.to_owned());
    client
        .send(request)
        .await
        .map(|_| ())
        .map_err(|error| format!("Failed to remove room keyword: {error}"))
}

async fn get_room_keywords(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    room_id_raw: &str,
) -> Result<MatrixRoomKeywordsResponse, String> {
    let room_id = RoomId::parse(room_id_raw)
        .map_err(|error| format!("Invalid room ID `{room_id_raw}`: {error}"))?;

    let client = require_client(paths, app_db, auth_state).await?;
    let settings = client.notification_settings().await;
    let keywords = room_keywords(&settings, &room_id).await?;

    Ok(MatrixRoomKeywordsResponse {
        room_id: room_id.to_string(),
        keywords,
    })
}

async fn add_room_keyword(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixAddRoomKeywordRequest,
) -> Result<MatrixAddRoomKeywordResponse, String> {
    validate_keyword(&request.keyword)?;

    let room_id = RoomId::parse(&request.room_id)
        .map_err(|error| format!("Invalid room ID `{}`: {error}", request.room_id))?;
    let keyword = request.keyword.trim();

    let client = require_client(paths, app_db, auth_state).await?;
    let settings = client.notification_settings().await;

    create_room_keyword_rule(&settings, &room_id, keyword).await?;

    let keywords = room_keywords(&settings, &room_id).await?;
    Ok(MatrixAddRoomKeywordResponse {
        room_id: room_id.to_string(),
        keywords,
    })
}

async fn remove_room_keyword(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixRemoveRoomKeywordRequest,
) -> Result<MatrixRemoveRoomKeywordResponse, String> {
    validate_keyword(&request.keyword)?;

    let room_id = RoomId::parse(&request.room_id)
        .map_err(|error| format!("Invalid room ID `{}`: {error}", request.room_id))?;
    let keyword = request.keyword.trim();

    let client = require_client(paths, app_db, auth_state).await?;
    let settings = client.notification_settings().await;

    // A raw API delete is not reflected in the SDK's in-memory rules until
    // the next sync, so derive the result from the pre-delete list.
    let keywords: Vec<String> = room_keywords(&settings, &room_id)
        .await?
        .into_iter()
        .filter(|existing| existing.as_str() != keyword)
        .collect();

    for (kind, rule_id) in room_keyword_rule_ids(&settings.ruleset().await, &room_id) {
        if keyword_from_rule_id(&room_id, &rule_id).as_deref() == Some(keyword) {
            delete_rule(&client, &kind, &rule_id).await?;
        }
    }

    Ok(MatrixRemoveRoomKeywordResponse {
        room_id: room_id.to_string(),
        keywords,
    })
}

pub(crate) mod delegates {
    //! Narrow re-exports so the crate root can expose one function per command.

    use super::*;

    pub async fn get_room_keywords(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixGetRoomKeywordsRequest,
    ) -> Result<MatrixRoomKeywordsResponse, String> {
        super::get_room_keywords(paths, app_db, auth_state, &request.room_id).await
    }

    pub async fn add_room_keyword(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixAddRoomKeywordRequest,
    ) -> Result<MatrixAddRoomKeywordResponse, String> {
        super::add_room_keyword(paths, app_db, auth_state, request).await
    }

    pub async fn remove_room_keyword(
        paths: &Paths,
        app_db: &Arc<AppDb>,
        auth_state: &AuthState,
        request: &MatrixRemoveRoomKeywordRequest,
    ) -> Result<MatrixRemoveRoomKeywordResponse, String> {
        super::remove_room_keyword(paths, app_db, auth_state, request).await
    }
}
