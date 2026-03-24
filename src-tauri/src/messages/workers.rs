use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::OnceLock;

use log::{debug, warn};
use matrix_sdk::deserialized_responses::{TimelineEvent, VerificationState};
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::events::room::MediaSource;
use matrix_sdk::ruma::uint;
use serde_json::Value;

use crate::protocol::events_schema::parse_timeline_message;

use super::types::{
    MatrixChatMessage, MatrixGetChatMessagesResponse, MatrixMessageDecryptionStatus,
    MatrixMessageVerificationStatus,
};

static MEDIA_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

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
            let image_url = if parsed.message_type.as_deref() == Some("m.image") {
                resolve_image_cache_path(client, &event)
                    .await
                    .or(parsed.image_url)
            } else {
                parsed.image_url
            };

            messages.push(MatrixChatMessage {
                event_id: parsed.event_id,
                sender: parsed.sender,
                timestamp: parsed.timestamp,
                body: parsed.body,
                message_type: parsed.message_type,
                image_url,
                encrypted: parsed.encrypted,
                decryption_status: parsed.decryption_status,
                verification_status: parsed.verification_status,
            });
        }
    }

    (messages, had_utd)
}

async fn resolve_image_cache_path(client: &matrix_sdk::Client, event: &Value) -> Option<String> {
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

    persist_image_cache(&bytes, &mime_type, event)
}

fn persist_image_cache(bytes: &[u8], mime_type: &str, event: &Value) -> Option<String> {
    let cache_dir = media_cache_dir();
    if let Err(error) = fs::create_dir_all(&cache_dir) {
        warn!("Failed to initialize media cache directory: {error}");
        return None;
    }

    let extension = image_extension_from_mime(mime_type);
    let file_stem = image_cache_key(event, mime_type, bytes);
    let file_name = format!("{file_stem}.{extension}");
    let final_path = cache_dir.join(file_name);

    if final_path.exists() {
        return Some(final_path.to_string_lossy().to_string());
    }

    let temp_path = cache_dir.join(format!("{file_stem}.tmp"));
    if let Err(error) = fs::write(&temp_path, bytes) {
        warn!("Failed to write cached media file: {error}");
        return None;
    }

    if let Err(error) = fs::rename(&temp_path, &final_path) {
        let _ = fs::remove_file(&temp_path);
        if final_path.exists() {
            return Some(final_path.to_string_lossy().to_string());
        }
        warn!("Failed to finalize cached media file: {error}");
        return None;
    }

    Some(final_path.to_string_lossy().to_string())
}

fn media_cache_dir() -> PathBuf {
    MEDIA_CACHE_DIR
        .get_or_init(|| {
            let mut dir = std::env::temp_dir();
            dir.push("singularity");
            dir.push("media-cache");
            dir
        })
        .clone()
}

fn image_extension_from_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

fn image_cache_key(event: &Value, mime_type: &str, bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    event.get("event_id").and_then(Value::as_str).hash(&mut hasher);
    event
        .get("origin_server_ts")
        .and_then(Value::as_u64)
        .hash(&mut hasher);
    event
        .get("room_id")
        .and_then(Value::as_str)
        .hash(&mut hasher);
    image_source_key(event).hash(&mut hasher);
    mime_type.hash(&mut hasher);

    // Include content size so messages without event_id still get stable keys per content.
    bytes.len().hash(&mut hasher);

    format!("img-{:016x}", hasher.finish())
}

fn image_source_key(event: &Value) -> Option<&str> {
    event
        .get("content")
        .and_then(|content| content.get("url"))
        .and_then(Value::as_str)
        .or_else(|| {
            event
                .get("content")
                .and_then(|content| content.get("file"))
                .and_then(|file| file.get("url"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            event
                .get("content")
                .and_then(|content| content.get("info"))
                .and_then(|info| info.get("thumbnail_url"))
                .and_then(Value::as_str)
        })
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
