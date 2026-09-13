pub mod formatting;
pub mod media;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use matrix_sdk::ruma::events::relation::InReplyTo;
use matrix_sdk::ruma::events::room::message::{
    FileMessageEventContent, ImageMessageEventContent, MessageType, Relation,
    RoomMessageEventContent, VideoMessageEventContent,
};

use protocol::{parse_event_id, parse_room_id};
use types::chat::MatrixPickerCustomEmoji;
use types::EventSink;

use media::{
    detect_media_kind, emit_transcode_progress, prepare_image_upload, prepare_video_upload,
    transmission_progress_percent, MediaKind, PreparedUpload, VideoTranscodeMode,
};

#[derive(Default)]
pub struct MediaTranscodeCancellationState {
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl MediaTranscodeCancellationState {
    fn job_key(room_id_raw: &str, file_path_raw: &str) -> String {
        format!("{room_id_raw}|{file_path_raw}")
    }

    pub fn register_job(&self, room_id_raw: &str, file_path_raw: &str) -> Arc<AtomicBool> {
        let key = Self::job_key(room_id_raw, file_path_raw);
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.insert(key, flag.clone());
        }
        flag
    }

    fn clear_job(&self, room_id_raw: &str, file_path_raw: &str) {
        let key = Self::job_key(room_id_raw, file_path_raw);
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.remove(key.as_str());
        }
    }

    pub fn cancel_job(&self, room_id_raw: &str, file_path_raw: &str) -> bool {
        let key = Self::job_key(room_id_raw, file_path_raw);

        if let Ok(jobs) = self.jobs.lock() {
            if let Some(flag) = jobs.get(key.as_str()) {
                flag.store(true, Ordering::Relaxed);
                return true;
            }
        }

        false
    }
}

#[derive(Clone, Debug)]
pub struct MediaSendResult {
    pub room_id: String,
    pub event_id: String,
}

pub fn cancel_media_transcode(
    state: &MediaTranscodeCancellationState,
    room_id: &str,
    file_path: &str,
) -> bool {
    state.cancel_job(room_id, file_path)
}

pub fn build_display_formatted_body_from_custom_emoji(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    formatting::build_display_formatted_body_from_custom_emoji(body, picker_custom_emoji)
}

pub async fn send_room_message_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
    in_reply_to_event_id_raw: Option<&str>,
) -> Result<String, String> {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return Err(String::from("Message cannot be empty"));
    }

    let room_id = parse_room_id(room_id_raw)?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let formatted_body =
        formatting::build_formatted_body_from_custom_emoji(trimmed_body, picker_custom_emoji);

    let mut content = if let Some(formatted_body) = formatted_body.as_deref() {
        RoomMessageEventContent::text_html(trimmed_body, formatted_body)
    } else {
        RoomMessageEventContent::text_plain(trimmed_body)
    };

    if let Some(in_reply_to_event_id_raw) = in_reply_to_event_id_raw {
        let in_reply_to_event_id = parse_event_id(in_reply_to_event_id_raw)?;
        content.relates_to = Some(Relation::Reply(
            matrix_sdk::ruma::events::relation::Reply::new(InReplyTo::new(in_reply_to_event_id)),
        ));
    }

    let response = room
        .send(content)
        .await
        .map_err(|error| format!("Failed to send room message: {error}"))?;

    Ok(response.response.event_id.to_string())
}

pub async fn send_media_file_from_client(
    client: &matrix_sdk::Client,
    event_sink: &Arc<dyn EventSink>,
    cancellation_state: &MediaTranscodeCancellationState,
    room_id_raw: &str,
    file_path_raw: &str,
    compress_media: bool,
) -> Result<MediaSendResult, String> {
    let cancellation_flag = cancellation_state.register_job(room_id_raw, file_path_raw);
    let result = send_media_file_impl(
        client,
        event_sink,
        room_id_raw,
        file_path_raw,
        compress_media,
        cancellation_flag,
    )
    .await;
    cancellation_state.clear_job(room_id_raw, file_path_raw);
    result
}

async fn send_media_file_impl(
    client: &matrix_sdk::Client,
    event_sink: &Arc<dyn EventSink>,
    room_id_raw: &str,
    file_path_raw: &str,
    compress_media: bool,
    cancellation_flag: Arc<AtomicBool>,
) -> Result<MediaSendResult, String> {
    let file_path = Path::new(file_path_raw);
    if !file_path.exists() || !file_path.is_file() {
        return Err(format!("File does not exist: {file_path_raw}"));
    }

    let room_id = parse_room_id(room_id_raw)?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let original_file_name = file_path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("attachment")
        .to_owned();

    let bytes = tokio::fs::read(file_path)
        .await
        .map_err(|error| format!("Failed to read file: {error}"))?;

    let media_kind = detect_media_kind(file_path, &bytes);
    let upload = match media_kind {
        MediaKind::Image => {
            prepare_image_upload(
                event_sink,
                room_id_raw,
                file_path,
                &bytes,
                compress_media,
                cancellation_flag.clone(),
            )
            .await?
        }
        MediaKind::Video => {
            prepare_video_upload(
                event_sink,
                room_id_raw,
                file_path,
                compress_media,
                cancellation_flag.clone(),
            )
            .await?
        }
        MediaKind::File => PreparedUpload {
            bytes,
            content_type: media::parse_mime("application/octet-stream")?,
            file_name: original_file_name.clone(),
            transcode_mode: VideoTranscodeMode::Software,
        },
    };

    let PreparedUpload {
        bytes,
        content_type,
        file_name,
        transcode_mode,
    } = upload;

    emit_transcode_progress(
        event_sink,
        room_id_raw,
        file_path,
        "uploading",
        0.0,
        transcode_mode,
    )?;

    let upload_request = room.client().media().upload(&content_type, bytes, None);
    let mut send_progress = upload_request.subscribe_to_send_progress();

    let upload_progress_event_sink = event_sink.clone();
    let upload_progress_room_id = room_id_raw.to_owned();
    let upload_progress_file_path = file_path.to_path_buf();

    let upload_progress_task = tokio::spawn(async move {
        while let Some(progress) = send_progress.next().await {
            let _ = emit_transcode_progress(
                &upload_progress_event_sink,
                upload_progress_room_id.as_str(),
                upload_progress_file_path.as_path(),
                "uploading",
                transmission_progress_percent(progress),
                transcode_mode,
            );
        }
    });

    let upload_response = upload_request
        .await
        .map_err(|error| format!("Failed to upload media: {error}"))?;

    let _ = upload_progress_task.await;

    emit_transcode_progress(
        event_sink,
        room_id_raw,
        file_path,
        "uploading",
        100.0,
        transcode_mode,
    )?;

    let content = match media_kind {
        MediaKind::Image => {
            let mut content = RoomMessageEventContent::new(MessageType::Image(
                ImageMessageEventContent::plain(file_name.clone(), upload_response.content_uri),
            ));
            if let MessageType::Image(image) = &mut content.msgtype {
                image.filename = Some(file_name.clone());
            }
            content
        }
        MediaKind::Video => {
            let mut content = RoomMessageEventContent::new(MessageType::Video(
                VideoMessageEventContent::plain(file_name.clone(), upload_response.content_uri),
            ));
            if let MessageType::Video(video) = &mut content.msgtype {
                video.filename = Some(file_name.clone());
            }
            content
        }
        MediaKind::File => {
            let mut content = RoomMessageEventContent::new(MessageType::File(
                FileMessageEventContent::plain(file_name.clone(), upload_response.content_uri),
            ));
            if let MessageType::File(file) = &mut content.msgtype {
                file.filename = Some(file_name.clone());
            }
            content
        }
    };

    let response = room
        .send(content)
        .await
        .map_err(|error| format!("Failed to send media message: {error}"))?;

    Ok(MediaSendResult {
        room_id: room_id.to_string(),
        event_id: response.response.event_id.to_string(),
    })
}
