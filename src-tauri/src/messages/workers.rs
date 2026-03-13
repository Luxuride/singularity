use log::{debug, warn};
use matrix_sdk::deserialized_responses::VerificationState;
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::uint;
use serde_json::Value;

use crate::protocol::events_schema::parse_timeline_message;

use super::persistence::sort_messages_by_timestamp;
use super::types::{
    MatrixChatMessage, MatrixGetChatMessagesResponse, MatrixMessageDecryptionStatus,
    MatrixMessageVerificationStatus,
};

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

    let build_options = |from: Option<String>, limit: Option<u32>| {
        let mut options = MessagesOptions::new(Direction::Backward);
        options.from = from;
        options.limit = uint!(50);

        if let Some(limit) = limit {
            options.limit = limit.min(100).into();
        }

        options
    };

    let response = room
        .messages(build_options(from.clone(), limit))
        .await
        .map_err(|error| format!("Failed to read room messages: {error}"))?;

    let parse_chunk = |chunk: Vec<matrix_sdk::deserialized_responses::TimelineEvent>| {
        let mut messages = Vec::new();
        let mut had_utd = false;

        for timeline in chunk {
            let encryption_info = timeline.encryption_info();
            let is_utd = timeline.kind.is_utd();
            let decryption_status = if is_utd {
                MatrixMessageDecryptionStatus::UnableToDecrypt
            } else if encryption_info.is_some() {
                MatrixMessageDecryptionStatus::Decrypted
            } else {
                MatrixMessageDecryptionStatus::Plaintext
            };

            if is_utd {
                had_utd = true;
            }

            let verification_status = match encryption_info.map(|info| &info.verification_state) {
                Some(VerificationState::Verified) => MatrixMessageVerificationStatus::Verified,
                Some(VerificationState::Unverified(_)) => {
                    MatrixMessageVerificationStatus::Unverified
                }
                None => MatrixMessageVerificationStatus::Unknown,
            };

            let Ok(event) = timeline.raw().deserialize_as::<Value>() else {
                continue;
            };

            if let Some(parsed) =
                parse_timeline_message(&event, decryption_status, verification_status)
            {
                messages.push(MatrixChatMessage {
                    event_id: parsed.event_id,
                    sender: parsed.sender,
                    timestamp: parsed.timestamp,
                    body: parsed.body,
                    encrypted: parsed.encrypted,
                    decryption_status: parsed.decryption_status,
                    verification_status: parsed.verification_status,
                });
            }
        }

        (messages, had_utd)
    };

    let (mut messages, mut had_utd) = parse_chunk(response.chunk);
    let mut next_from = response.end;

    if had_utd && client.encryption().backups().are_enabled().await {
        if let Err(error) = client
            .encryption()
            .backups()
            .download_room_keys_for_room(&room_id)
            .await
        {
            warn!(
                "Failed to download backup keys for room {}: {}",
                room_id, error
            );
        } else if let Ok(retry_response) = room.messages(build_options(from, limit)).await {
            let (retry_messages, retry_had_utd) = parse_chunk(retry_response.chunk);
            messages = retry_messages;
            had_utd = retry_had_utd;
            next_from = retry_response.end;
        }
    }

    sort_messages_by_timestamp(&mut messages);
    debug!(
        "Fetched {} chat messages (utd_present={})",
        messages.len(),
        had_utd
    );

    Ok(MatrixGetChatMessagesResponse {
        room_id: room_id.to_string(),
        next_from,
        messages,
    })
}

pub(crate) async fn send_room_message_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    body: &str,
) -> Result<String, String> {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return Err(String::from("Message cannot be empty"));
    }

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
        .map_err(|_| String::from("roomId is invalid"))?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let response = room
        .send(RoomMessageEventContent::text_plain(trimmed_body))
        .await
        .map_err(|error| format!("Failed to send room message: {error}"))?;

    Ok(response.event_id.to_string())
}
