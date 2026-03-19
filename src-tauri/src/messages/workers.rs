use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use log::{debug, warn};
use matrix_sdk::deserialized_responses::{TimelineEvent, VerificationState};
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::events::room::MediaSource;
use matrix_sdk::ruma::uint;
use serde_json::{json, Value};

use crate::protocol::event_types;
use crate::protocol::events_schema::parse_timeline_message;

use super::streaming::VideoStreamState;
use super::types::{
    MatrixChatMessage, MatrixGetChatMessagesResponse, MatrixMessageDecryptionStatus,
    MatrixMessageVerificationStatus, MatrixPrepareVideoPlaybackResponse,
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

    let response = room
        .messages(build_messages_options(from.clone(), limit))
        .await
        .map_err(|error| format!("Failed to read room messages: {error}"))?;

    let (mut messages, mut had_utd) = parse_message_chunk(client, response.chunk).await;
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
        } else if let Ok(retry_response) = room.messages(build_messages_options(from, limit)).await
        {
            let (retry_messages, retry_had_utd) =
                parse_message_chunk(client, retry_response.chunk).await;
            messages = retry_messages;
            had_utd = retry_had_utd;
            next_from = retry_response.end;
        }
    }

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

pub(crate) async fn prepare_video_playback_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    event_id_raw: &str,
    stream_state: &VideoStreamState,
) -> Result<MatrixPrepareVideoPlaybackResponse, String> {
    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
        .map_err(|_| String::from("roomId is invalid"))?;

    let event_id = matrix_sdk::ruma::OwnedEventId::try_from(event_id_raw)
        .map_err(|_| String::from("eventId is invalid"))?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let event = room
        .event(&event_id, None)
        .await
        .map_err(|error| format!("Failed to fetch event for video playback: {error}"))?;

    let event_json = event
        .raw()
        .deserialize_as::<Value>()
        .map_err(|error| format!("Failed to decode event payload for video playback: {error}"))?;

    let msgtype = event_json
        .get("content")
        .and_then(|content| content.get("msgtype"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    if !is_video_message_content(msgtype, &event_json) {
        return Err(String::from("Event is not a video message"));
    }

    let media_source = video_media_source_from_event(&event_json)
        .ok_or_else(|| String::from("Video media source is missing or invalid"))?;

    let mime_type = video_mime_type_from_event(&event_json)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("application/octet-stream"));

    let stream_url = stream_state.create_stream_url(media_source, mime_type)?;

    Ok(MatrixPrepareVideoPlaybackResponse { stream_url })
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

async fn parse_message_chunk(
    client: &matrix_sdk::Client,
    chunk: Vec<TimelineEvent>,
) -> (Vec<MatrixChatMessage>, bool) {
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
            Some(VerificationState::Unverified(_)) => MatrixMessageVerificationStatus::Unverified,
            None => MatrixMessageVerificationStatus::Unknown,
        };

        let Ok(event) = timeline.raw().deserialize_as::<Value>() else {
            continue;
        };

        if let Some(parsed) = parse_timeline_message(
            &event,
            &client.homeserver(),
            decryption_status,
            verification_status,
        ) {
            let image_url = if parsed.message_type.as_deref() == Some(event_types::message_types::IMAGE) {
                resolve_image_data_url(client, &event)
                    .await
                    .or(parsed.image_url)
            } else {
                parsed.image_url
            };

            let video_thumbnail_url = if parsed.message_type.as_deref() == Some(event_types::message_types::VIDEO) {
                resolve_video_thumbnail_data_url(client, &event)
                    .await
                    .or(parsed.video_thumbnail_url)
            } else {
                parsed.video_thumbnail_url
            };

            messages.push(MatrixChatMessage {
                event_id: parsed.event_id,
                sender: parsed.sender,
                timestamp: parsed.timestamp,
                body: parsed.body,
                message_type: parsed.message_type,
                image_url,
                video_thumbnail_url,
                video_mime_type: parsed.video_mime_type,
                video_size_bytes: parsed.video_size_bytes,
                video_duration_ms: parsed.video_duration_ms,
                encrypted: parsed.encrypted,
                decryption_status: parsed.decryption_status,
                verification_status: parsed.verification_status,
            });
        }
    }

    (messages, had_utd)
}

async fn resolve_image_data_url(client: &matrix_sdk::Client, event: &Value) -> Option<String> {
    let media_source = image_media_source_from_event(event)?;
    let mime_type = image_mime_type_from_event(event)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("application/octet-stream"));

    let request = MediaRequestParameters {
        source: media_source,
        format: MediaFormat::File,
    };

    let bytes = match client.media().get_media_content(&request, true).await {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!("Failed to fetch image media content: {error}");
            return None;
        }
    };

    let encoded = BASE64_STANDARD.encode(bytes);
    Some(format!("data:{mime_type};base64,{encoded}"))
}

fn image_media_source_from_event(event: &Value) -> Option<MediaSource> {
    let content = event.get("content")?;
    serde_json::from_value(content.clone()).ok()
}

fn image_mime_type_from_event(event: &Value) -> Option<String> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("mimetype"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

async fn resolve_video_thumbnail_data_url(
    client: &matrix_sdk::Client,
    event: &Value,
) -> Option<String> {
    let media_source = video_thumbnail_media_source_from_event(event)?;
    let mime_type = video_thumbnail_mime_type_from_event(event)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("application/octet-stream"));

    let request = MediaRequestParameters {
        source: media_source,
        format: MediaFormat::File,
    };

    let bytes = match client.media().get_media_content(&request, true).await {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!("Failed to fetch video thumbnail media content: {error}");
            return None;
        }
    };

    let encoded = BASE64_STANDARD.encode(bytes);
    Some(format!("data:{mime_type};base64,{encoded}"))
}

fn video_thumbnail_media_source_from_event(event: &Value) -> Option<MediaSource> {
    let info = event.get("content")?.get("info")?;

    if let Some(thumbnail_url) = info.get("thumbnail_url").and_then(Value::as_str) {
        let candidate = json!({ "url": thumbnail_url });
        return serde_json::from_value(candidate).ok();
    }

    if let Some(thumbnail_file) = info.get("thumbnail_file") {
        let candidate = json!({ "file": thumbnail_file });
        return serde_json::from_value(candidate).ok();
    }

    None
}

fn video_thumbnail_mime_type_from_event(event: &Value) -> Option<String> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("thumbnail_info"))
        .and_then(|thumbnail_info| thumbnail_info.get("mimetype"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn video_media_source_from_event(event: &Value) -> Option<MediaSource> {
    let content = event.get("content")?;
    serde_json::from_value(content.clone()).ok()
}

fn video_mime_type_from_event(event: &Value) -> Option<String> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("mimetype"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn is_video_message_content(msgtype: &str, event: &Value) -> bool {
    if msgtype == event_types::message_types::VIDEO
        || msgtype == event_types::message_types::VIDEO_UNSTABLE
    {
        return true;
    }

    if msgtype != event_types::message_types::FILE && msgtype != event_types::message_types::IMAGE {
        return false;
    }

    video_mime_type_from_event(event)
        .map(|mimetype| mimetype.to_ascii_lowercase().starts_with("video/"))
        .unwrap_or(false)
}
