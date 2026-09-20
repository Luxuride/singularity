use std::borrow::Cow;

use arboard::{Clipboard, ImageData};
use matrix_sdk::Client;
use serde_json::Value;

use crate::emoji::load_picker_assets_from_client;
use crate::media::{DefaultMediaResolver, MediaResolver};
use crate::reactions::toggle_reaction_from_client;
use crate::receive::fetch_room_messages_from_client;
use crate::send::{
    build_display_formatted_body_from_custom_emoji, send_media_file_from_client,
    send_room_message_from_client, MediaTranscodeCancellationState,
};
use types::chat::{
    MatrixDownloadFileData, MatrixGetChatMessagesResponse, MatrixGetEmojiPacksResponse,
    MatrixGetUserAvatarResponse, MatrixPickerCustomEmoji, MatrixResolveVideoUrlResponse,
    MatrixSendChatMessageResponse, MatrixSendMediaFileResponse, MatrixStreamChatMessagesRequest,
    MatrixStreamChatMessagesResponse, MatrixToggleReactionResponse,
};
use types::EventSink;

pub async fn get_chat_messages(
    client: &Client,
    room_id: &str,
    from: Option<String>,
    limit: Option<u32>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    fetch_room_messages_from_client(client, room_id, from, limit).await
}

pub async fn stream_chat_messages(
    context: crate::receive::StreamRoomMessagesContext<'_>,
    request: MatrixStreamChatMessagesRequest,
) -> Result<MatrixStreamChatMessagesResponse, String> {
    crate::receive::stream_room_messages_from_client(context, request).await
}

pub async fn send_chat_message(
    client: &Client,
    room_id: &str,
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
    in_reply_to_event_id: Option<&str>,
) -> Result<MatrixSendChatMessageResponse, String> {
    let event_id = send_room_message_from_client(
        client,
        room_id,
        body,
        picker_custom_emoji,
        in_reply_to_event_id,
    )
    .await?;

    let formatted_body = build_display_formatted_body_from_custom_emoji(body, picker_custom_emoji);

    Ok(MatrixSendChatMessageResponse {
        event_id,
        formatted_body,
    })
}

pub async fn send_media_file(
    client: &Client,
    event_sink: &std::sync::Arc<dyn EventSink>,
    cancellation_state: &MediaTranscodeCancellationState,
    room_id: &str,
    file_path: &str,
    compress_media: bool,
) -> Result<MatrixSendMediaFileResponse, String> {
    let result = send_media_file_from_client(
        client,
        event_sink,
        cancellation_state,
        room_id,
        file_path,
        compress_media,
    )
    .await?;
    Ok(MatrixSendMediaFileResponse {
        event_id: result.event_id,
    })
}

pub async fn cancel_media_transcode(
    state: &MediaTranscodeCancellationState,
    room_id: &str,
    file_path: &str,
) -> bool {
    crate::send::cancel_media_transcode(state, room_id, file_path)
}

pub async fn toggle_reaction(
    client: &Client,
    room_id: &str,
    target_event_id: &str,
    key: &str,
) -> Result<MatrixToggleReactionResponse, String> {
    let (added, event_id) =
        toggle_reaction_from_client(client, room_id, target_event_id, key).await?;
    Ok(MatrixToggleReactionResponse { added, event_id })
}

pub async fn get_emoji_packs(client: &Client) -> Result<MatrixGetEmojiPacksResponse, String> {
    let custom_emoji = load_picker_assets_from_client(client).await?;
    Ok(MatrixGetEmojiPacksResponse { custom_emoji })
}

pub async fn get_user_avatar(
    client: &Client,
    room_id: &str,
    user_id: &str,
) -> Result<MatrixGetUserAvatarResponse, String> {
    let user_id = protocol::parse_user_id(user_id)?;
    let room_id = protocol::parse_room_id(room_id)?;

    let room = match client.get_room(&room_id) {
        Some(room) => room,
        None => {
            return Ok(MatrixGetUserAvatarResponse {
                user_id: user_id.to_string(),
                image_url: None,
            });
        }
    };

    let image_url = match room.get_member(user_id.as_ref()).await {
        Ok(Some(member)) => match member.avatar_url() {
            Some(avatar_url) => {
                crate::media::cache_mxc_media_to_local_path(client, avatar_url.as_str()).await
            }
            None => None,
        },
        Ok(None) => None,
        Err(_) => None,
    };

    Ok(MatrixGetUserAvatarResponse {
        user_id: user_id.to_string(),
        image_url,
    })
}

pub async fn copy_image_to_clipboard(image_url: &str) -> Result<(), String> {
    let image_bytes =
        if let Some(bytes) = assets::image::load_media_bytes_from_resolved_url(image_url) {
            bytes
        } else if image_url.starts_with("http://") || image_url.starts_with("https://") {
            let response = reqwest::get(image_url)
                .await
                .map_err(|error| format!("Failed to fetch image URL: {error}"))?;

            if !response.status().is_success() {
                return Err(format!(
                    "Failed to fetch image URL with status {}",
                    response.status()
                ));
            }

            response
                .bytes()
                .await
                .map_err(|error| format!("Failed to read image URL response: {error}"))?
                .to_vec()
        } else {
            return Err(String::from("Unsupported image URL scheme"));
        };

    if image_bytes.is_empty() {
        return Err(String::from("Image bytes are empty"));
    }

    let decoded = image::load_from_memory(image_bytes.as_slice())
        .map_err(|error| format!("Failed to decode image bytes: {error}"))?;

    let rgba = decoded.to_rgba8();
    let width =
        usize::try_from(rgba.width()).map_err(|_| String::from("Image width is out of range"))?;
    let height =
        usize::try_from(rgba.height()).map_err(|_| String::from("Image height is out of range"))?;

    let mut clipboard =
        Clipboard::new().map_err(|error| format!("Failed to initialize clipboard: {error}"))?;
    clipboard
        .set_image(ImageData {
            width,
            height,
            bytes: Cow::Owned(rgba.into_raw()),
        })
        .map_err(|error| format!("Failed to write image to clipboard: {error}"))
}

pub async fn read_clipboard_text() -> Result<String, String> {
    let mut clipboard =
        Clipboard::new().map_err(|error| format!("Failed to initialize clipboard: {error}"))?;
    clipboard
        .get_text()
        .map_err(|error| format!("Failed to read clipboard text: {error}"))
}

/// Resolve the local cache path for a video message's media, downloading it on
/// demand. Returns `None` when the event is not a video or has no media.
pub async fn resolve_video_url(
    client: &Client,
    room_id: &str,
    event_id: &str,
) -> Result<MatrixResolveVideoUrlResponse, String> {
    let room_id = protocol::parse_room_id(room_id)?;
    let event_id = protocol::parse_event_id(event_id)?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let timeline_event = room
        .event(&event_id, None)
        .await
        .map_err(|error| format!("Failed to fetch video event: {error}"))?;

    let Ok(event) = timeline_event.raw().deserialize_as::<Value>() else {
        return Ok(MatrixResolveVideoUrlResponse { video_url: None });
    };

    let message_type = event
        .get("content")
        .and_then(|content| content.get("msgtype"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    if message_type != "m.video" {
        return Ok(MatrixResolveVideoUrlResponse { video_url: None });
    }

    let video_url = DefaultMediaResolver
        .resolve_video_cache_path(client, &event)
        .await;

    Ok(MatrixResolveVideoUrlResponse { video_url })
}

/// Download the raw bytes for a file message's media. Files are not cached;
/// the caller writes the bytes to a user-chosen destination path. Returns
/// `None` when the event is not a file or has no media.
pub async fn download_file_data(
    client: &Client,
    room_id: &str,
    event_id: &str,
) -> Result<Option<MatrixDownloadFileData>, String> {
    let room_id = protocol::parse_room_id(room_id)?;
    let event_id = protocol::parse_event_id(event_id)?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let timeline_event = room
        .event(&event_id, None)
        .await
        .map_err(|error| format!("Failed to fetch file event: {error}"))?;

    let Ok(event) = timeline_event.raw().deserialize_as::<Value>() else {
        return Ok(None);
    };

    let Some(content) = event.get("content") else {
        return Ok(None);
    };

    let message_type = content
        .get("msgtype")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if message_type != "m.file" {
        return Ok(None);
    }

    let Some(bytes) = DefaultMediaResolver
        .download_file_bytes(client, &event)
        .await
    else {
        return Ok(None);
    };

    let file_name = content
        .get("filename")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let mime_type = content
        .get("info")
        .and_then(|info| info.get("mimetype"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    Ok(Some(MatrixDownloadFileData {
        bytes,
        file_name,
        mime_type,
    }))
}
