use log::debug;
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::uint;
use serde_json::Value;

use crate::protocol::events_schema::parse_timeline_message;

use super::persistence::sort_messages_by_timestamp;
use super::types::{MatrixChatMessage, MatrixGetChatMessagesResponse};

pub(crate) async fn fetch_room_messages_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    from: Option<String>,
    limit: Option<u32>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
        .map_err(|_| String::from("roomId is invalid"))?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let mut options = MessagesOptions::new(Direction::Backward);
    if let Some(from) = from {
        options.from = Some(from);
    }

    options.limit = uint!(50);
    if let Some(limit) = limit {
        options.limit = limit.min(100).into();
    }

    let response = room
        .messages(options)
        .await
        .map_err(|error| format!("Failed to read room messages: {error}"))?;

    let mut messages = Vec::new();
    for timeline in response.chunk {
        let is_encrypted_event = timeline.encryption_info().is_some() || timeline.kind.is_utd();
        let Ok(event) = timeline.raw().deserialize_as::<Value>() else {
            continue;
        };

        if let Some(parsed) = parse_timeline_message(&event, is_encrypted_event) {
            messages.push(MatrixChatMessage {
                event_id: parsed.event_id,
                sender: parsed.sender,
                timestamp: parsed.timestamp,
                body: parsed.body,
                encrypted: parsed.encrypted,
            });
        }
    }

    sort_messages_by_timestamp(&mut messages);
    debug!("Fetched {} chat messages", messages.len());

    Ok(MatrixGetChatMessagesResponse {
        room_id: room_id.to_string(),
        next_from: response.end,
        messages,
    })
}
