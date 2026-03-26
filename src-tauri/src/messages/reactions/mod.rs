use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::events::reaction::ReactionEventContent;
use matrix_sdk::ruma::events::relation::Annotation;
use matrix_sdk::ruma::uint;
use serde_json::Value;

use crate::protocol::events_schema::parse_reaction_event;

pub(crate) trait ReactionManager {
    async fn toggle_reaction(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        target_event_id_raw: &str,
        key: &str,
    ) -> Result<(bool, Option<String>), String>;
}

#[derive(Default, Clone, Copy)]
pub(crate) struct MatrixReactionManager;

impl ReactionManager for MatrixReactionManager {
    async fn toggle_reaction(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        target_event_id_raw: &str,
        key: &str,
    ) -> Result<(bool, Option<String>), String> {
        let reaction_key = key.trim();
        if reaction_key.is_empty() {
            return Err(String::from("Reaction key cannot be empty"));
        }

        let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
            .map_err(|_| String::from("roomId is invalid"))?;
        let target_event_id = matrix_sdk::ruma::OwnedEventId::try_from(target_event_id_raw)
            .map_err(|_| String::from("targetEventId is invalid"))?;

        let room = client
            .get_room(&room_id)
            .ok_or_else(|| String::from("Room is not available in current session"))?;

        let own_user_id = client
            .user_id()
            .ok_or_else(|| String::from("Session user ID is unavailable"))?
            .to_string();

        if let Some(existing_event_id) = find_matching_own_reaction_event_id(
            &room,
            &own_user_id,
            target_event_id_raw,
            reaction_key,
        )
        .await?
        {
            let redact_target = matrix_sdk::ruma::OwnedEventId::try_from(existing_event_id.clone())
                .map_err(|_| String::from("Found invalid reaction event id for redaction"))?;

            room.redact(&redact_target, Some("Toggle reaction off"), None)
                .await
                .map_err(|error| format!("Failed to remove reaction: {error}"))?;

            return Ok((false, Some(existing_event_id)));
        }

        let content =
            ReactionEventContent::new(Annotation::new(target_event_id, reaction_key.to_owned()));
        let response = room
            .send(content)
            .await
            .map_err(|error| format!("Failed to send reaction: {error}"))?;

        Ok((true, Some(response.event_id.to_string())))
    }
}

pub(crate) async fn toggle_reaction_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    target_event_id_raw: &str,
    key: &str,
) -> Result<(bool, Option<String>), String> {
    MatrixReactionManager
        .toggle_reaction(client, room_id_raw, target_event_id_raw, key)
        .await
}

async fn find_matching_own_reaction_event_id(
    room: &matrix_sdk::Room,
    own_user_id: &str,
    target_event_id: &str,
    key: &str,
) -> Result<Option<String>, String> {
    let response = room
        .messages(build_messages_options(None, Some(100)))
        .await
        .map_err(|error| format!("Failed to read room messages for reaction toggle: {error}"))?;

    for timeline in response.chunk {
        let Ok(event) = timeline.raw().deserialize_as::<Value>() else {
            continue;
        };

        let Some(parsed) = parse_reaction_event(&event) else {
            continue;
        };

        if parsed.sender != own_user_id {
            continue;
        }

        if parsed.target_event_id != target_event_id {
            continue;
        }

        if parsed.key != key {
            continue;
        }

        if let Some(event_id) = parsed.event_id {
            return Ok(Some(event_id));
        }
    }

    Ok(None)
}

fn build_messages_options(from: Option<String>, limit: Option<u32>) -> MessagesOptions {
    let mut options = MessagesOptions::new(Direction::Backward);
    options.from = from;
    options.limit = uint!(50);
    if let Some(limit) = limit {
        options.limit = limit.min(100).into();
    }
    options
}
