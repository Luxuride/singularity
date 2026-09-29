pub mod formatting;
pub mod media;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use matrix_sdk::ruma::events::relation::InReplyTo;
use matrix_sdk::ruma::events::room::message::{
    FileMessageEventContent, ImageMessageEventContent, MessageType, Relation,
    RoomMessageEventContent, VideoInfo, VideoMessageEventContent,
};

use protocol::{parse_event_id, parse_room_id};
use types::chat::MatrixPickerCustomEmoji;
use types::EventSink;

use media::{
    detect_media_kind, emit_transcode_progress, prepare_image_upload, prepare_video_upload,
    report_transcode_progress, transmission_progress_percent, MediaKind, PreparedUpload,
    VideoTranscodeMode,
};

#[derive(Default)]
pub struct MediaTranscodeCancellationState {
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
    next_job_id: AtomicU64,
}

/// Handle for one in-flight send. A send clears only its own entry.
#[derive(Clone, Debug)]
pub struct MediaTranscodeJob {
    key: String,
    flag: Arc<AtomicBool>,
}

impl MediaTranscodeJob {
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }
}

impl MediaTranscodeCancellationState {
    fn job_key(room_id_raw: &str, file_path_raw: &str) -> String {
        format!("{room_id_raw}|{file_path_raw}")
    }

    /// Registers a job under a key unique to this call, so concurrent sends of
    /// the same file to the same room each get their own flag and entry.
    pub fn register_job(&self, room_id_raw: &str, file_path_raw: &str) -> MediaTranscodeJob {
        let key = format!(
            "{}#{}",
            Self::job_key(room_id_raw, file_path_raw),
            self.next_job_id.fetch_add(1, Ordering::Relaxed),
        );
        let flag = Arc::new(AtomicBool::new(false));

        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.insert(key.clone(), flag.clone());
        }

        MediaTranscodeJob { key, flag }
    }

    fn clear_job(&self, job: &MediaTranscodeJob) {
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.remove(job.key.as_str());
        }
    }

    /// Cancels every in-flight send of this file in this room. The UI's cancel
    /// button addresses a send by its file, not by an id it never received.
    pub fn cancel_job(&self, room_id_raw: &str, file_path_raw: &str) -> bool {
        let key_prefix = Self::job_key(room_id_raw, file_path_raw);
        let mut cancelled = false;

        if let Ok(jobs) = self.jobs.lock() {
            for (key, flag) in jobs.iter() {
                if key.starts_with(&key_prefix) {
                    flag.store(true, Ordering::Relaxed);
                    cancelled = true;
                }
            }
        }

        cancelled
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
    let job = cancellation_state.register_job(room_id_raw, file_path_raw);
    let result = send_media_file_impl(
        client,
        event_sink,
        room_id_raw,
        file_path_raw,
        compress_media,
        &job,
    )
    .await;
    cancellation_state.clear_job(&job);
    result
}

async fn send_media_file_impl(
    client: &matrix_sdk::Client,
    event_sink: &Arc<dyn EventSink>,
    room_id_raw: &str,
    file_path_raw: &str,
    compress_media: bool,
    job: &MediaTranscodeJob,
) -> Result<MediaSendResult, String> {
    let cancellation_flag = job.flag();
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
    if cancellation_flag.load(Ordering::Relaxed) {
        return Err(String::from("Media send cancelled by user"));
    }

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
            content_type: media::guess_mime_from_extension(&original_file_name)?,
            bytes,
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

    report_transcode_progress(
        event_sink,
        room_id_raw,
        file_path,
        "uploading",
        0.0,
        transcode_mode,
    );

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

    let upload_response = upload_request.await;
    let _ = upload_progress_task.await;

    // A cancel that lands during the upload still stops the send: the media is
    // already on the server here, so returning success would post an attachment
    // the user asked not to send.
    if cancellation_flag.load(Ordering::Relaxed) {
        let _ = upload_response;
        return Err(String::from("Media send cancelled by user"));
    }

    let upload_response =
        upload_response.map_err(|error| format!("Failed to upload media: {error}"))?;

    report_transcode_progress(
        event_sink,
        room_id_raw,
        file_path,
        "uploading",
        100.0,
        transcode_mode,
    );

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
            let mut video_info = VideoInfo::default();
            video_info.mimetype = Some(content_type.to_string());
            let video =
                VideoMessageEventContent::plain(file_name.clone(), upload_response.content_uri)
                    .info(Box::new(video_info));
            let mut content = RoomMessageEventContent::new(MessageType::Video(video));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_sends_of_one_file_get_independent_flags() {
        let state = MediaTranscodeCancellationState::default();

        let first = state.register_job("!room:example.org", "/tmp/clip.mp4");
        let second = state.register_job("!room:example.org", "/tmp/clip.mp4");

        assert!(state.cancel_job("!room:example.org", "/tmp/clip.mp4"));

        assert!(first.flag().load(Ordering::Relaxed));
        assert!(second.flag().load(Ordering::Relaxed));
    }

    #[test]
    fn clearing_one_send_leaves_the_other_cancellable() {
        let state = MediaTranscodeCancellationState::default();

        let first = state.register_job("!room:example.org", "/tmp/clip.mp4");
        let second = state.register_job("!room:example.org", "/tmp/clip.mp4");
        state.clear_job(&first);

        assert!(state.cancel_job("!room:example.org", "/tmp/clip.mp4"));
        assert!(!first.flag().load(Ordering::Relaxed));
        assert!(second.flag().load(Ordering::Relaxed));
    }

    #[test]
    fn cancelling_an_unknown_file_reports_nothing_to_cancel() {
        let state = MediaTranscodeCancellationState::default();
        state.register_job("!room:example.org", "/tmp/clip.mp4");

        assert!(!state.cancel_job("!room:example.org", "/tmp/other.mp4"));
    }

    #[test]
    fn a_cleared_job_is_no_longer_cancellable() {
        let state = MediaTranscodeCancellationState::default();
        let job = state.register_job("!room:example.org", "/tmp/clip.mp4");
        state.clear_job(&job);

        assert!(!state.cancel_job("!room:example.org", "/tmp/clip.mp4"));
    }
}
