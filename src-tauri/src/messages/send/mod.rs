use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use matrix_sdk::ruma::events::relation::InReplyTo;
use matrix_sdk::ruma::events::room::message::{
    FileMessageEventContent, ImageMessageEventContent, MessageType, Relation,
    RoomMessageEventContent, VideoMessageEventContent,
};
use matrix_sdk::TransmissionProgress;
use mime::Mime;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::mpsc;

use super::types::MatrixPickerCustomEmoji;
use crate::assets::image::media_cache_dir_path;
use crate::messages::types::MatrixMediaTranscodeProgressEvent;
use crate::protocol::event_paths;
use crate::protocol::{parse_event_id, parse_room_id};
mod formatting;
use formatting::{
    build_display_formatted_body_from_custom_emoji, build_formatted_body_from_custom_emoji,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VideoTranscodeMode {
    Vaapi,
    Cuda,
    Software,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VideoCodec {
    Vp9,
    Vp8,
    H264,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VideoTranscodePlan {
    mode: VideoTranscodeMode,
    codec: VideoCodec,
}

pub(crate) trait MessageSender {
    async fn send_room_message(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        body: &str,
        picker_custom_emoji: &[MatrixPickerCustomEmoji],
        in_reply_to_event_id_raw: Option<&str>,
    ) -> Result<String, String>;

    async fn send_media_file(
        &self,
        client: &matrix_sdk::Client,
        app_handle: &AppHandle,
        cancellation_state: &MediaTranscodeCancellationState,
        room_id_raw: &str,
        file_path_raw: &str,
        compress_media: bool,
    ) -> Result<MediaSendResult, String>;
}

#[derive(Clone, Debug)]
pub(crate) struct MediaSendResult {
    pub room_id: String,
    pub event_id: String,
}

#[derive(Clone, Debug)]
enum MediaKind {
    Image,
    Video,
    File,
}

#[derive(Default)]
pub(crate) struct MediaTranscodeCancellationState {
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl MediaTranscodeCancellationState {
    fn job_key(room_id_raw: &str, file_path_raw: &str) -> String {
        format!("{room_id_raw}|{file_path_raw}")
    }

    fn register_job(&self, room_id_raw: &str, file_path_raw: &str) -> Arc<AtomicBool> {
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

    fn cancel_job(&self, room_id_raw: &str, file_path_raw: &str) -> bool {
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

#[derive(Clone)]
struct PreparedUpload {
    bytes: Vec<u8>,
    content_type: Mime,
    file_name: String,
    transcode_mode: VideoTranscodeMode,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct MatrixMessageSender;

impl MessageSender for MatrixMessageSender {
    async fn send_room_message(
        &self,
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
            build_formatted_body_from_custom_emoji(trimmed_body, picker_custom_emoji);

        let mut content = if let Some(formatted_body) = formatted_body.as_deref() {
            RoomMessageEventContent::text_html(trimmed_body, formatted_body)
        } else {
            RoomMessageEventContent::text_plain(trimmed_body)
        };

        if let Some(in_reply_to_event_id_raw) = in_reply_to_event_id_raw {
            let in_reply_to_event_id = parse_event_id(in_reply_to_event_id_raw)?;
            content.relates_to = Some(Relation::Reply {
                in_reply_to: InReplyTo::new(in_reply_to_event_id),
            });
        }

        let response = room
            .send(content)
            .await
            .map_err(|error| format!("Failed to send room message: {error}"))?;

        Ok(response.event_id.to_string())
    }

    async fn send_media_file(
        &self,
        client: &matrix_sdk::Client,
        app_handle: &AppHandle,
        cancellation_state: &MediaTranscodeCancellationState,
        room_id_raw: &str,
        file_path_raw: &str,
        compress_media: bool,
    ) -> Result<MediaSendResult, String> {
        let cancellation_flag = cancellation_state.register_job(room_id_raw, file_path_raw);
        let result = self
            .send_media_file_impl(
                client,
                app_handle,
                room_id_raw,
                file_path_raw,
                compress_media,
                cancellation_flag,
            )
            .await;
        cancellation_state.clear_job(room_id_raw, file_path_raw);
        result
    }
}

impl MatrixMessageSender {
    async fn send_media_file_impl(
        &self,
        client: &matrix_sdk::Client,
        app_handle: &AppHandle,
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
                    app_handle,
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
                    app_handle,
                    room_id_raw,
                    file_path,
                    compress_media,
                    cancellation_flag.clone(),
                )
                .await?
            }
            MediaKind::File => PreparedUpload {
                bytes,
                content_type: parse_mime("application/octet-stream")?,
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
            app_handle,
            room_id_raw,
            file_path,
            "uploading",
            0.0,
            transcode_mode,
        )?;

        let upload_request = room.client().media().upload(&content_type, bytes, None);
        let mut send_progress = upload_request.subscribe_to_send_progress();

        let upload_progress_app_handle = app_handle.clone();
        let upload_progress_room_id = room_id_raw.to_owned();
        let upload_progress_file_path = file_path.to_path_buf();

        let upload_progress_task = tokio::spawn(async move {
            while let Some(progress) = send_progress.next().await {
                let _ = emit_transcode_progress(
                    &upload_progress_app_handle,
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
            app_handle,
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
            event_id: response.event_id.to_string(),
        })
    }
}

pub(crate) async fn send_room_message_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
    in_reply_to_event_id_raw: Option<&str>,
) -> Result<String, String> {
    MatrixMessageSender
        .send_room_message(
            client,
            room_id_raw,
            body,
            picker_custom_emoji,
            in_reply_to_event_id_raw,
        )
        .await
}

pub(crate) fn build_display_formatted_body_from_custom_emoji_for_send(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    build_display_formatted_body_from_custom_emoji(body, picker_custom_emoji)
}

pub(crate) async fn send_media_file_from_client(
    client: &matrix_sdk::Client,
    app_handle: &AppHandle,
    cancellation_state: &MediaTranscodeCancellationState,
    room_id_raw: &str,
    file_path_raw: &str,
    compress_media: bool,
) -> Result<MediaSendResult, String> {
    MatrixMessageSender
        .send_media_file(
            client,
            app_handle,
            cancellation_state,
            room_id_raw,
            file_path_raw,
            compress_media,
        )
        .await
}

pub(crate) fn cancel_media_transcode(
    cancellation_state: &MediaTranscodeCancellationState,
    room_id_raw: &str,
    file_path_raw: &str,
) -> bool {
    cancellation_state.cancel_job(room_id_raw, file_path_raw)
}

fn detect_media_kind(path: &Path, bytes: &[u8]) -> MediaKind {
    if image::guess_format(bytes).is_ok() {
        return MediaKind::Image;
    }

    match path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("mp4") | Some("mkv") | Some("mov") | Some("webm") | Some("avi") => MediaKind::Video,
        _ => MediaKind::File,
    }
}

fn parse_mime(raw: &str) -> Result<Mime, String> {
    raw.parse::<Mime>()
        .map_err(|error| format!("Invalid mime type {raw}: {error}"))
}

fn guess_image_mime(path: &Path) -> Result<Mime, String> {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => parse_mime("image/png"),
        Some("gif") => parse_mime("image/gif"),
        Some("bmp") => parse_mime("image/bmp"),
        Some("jpg") | Some("jpeg") => parse_mime("image/jpeg"),
        Some("webp") => parse_mime("image/webp"),
        _ => parse_mime("image/webp"),
    }
}

fn guess_video_mime(path: &Path) -> Result<Mime, String> {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("webm") => parse_mime("video/webm"),
        Some("mp4") => parse_mime("video/mp4"),
        Some("mov") => parse_mime("video/quicktime"),
        _ => parse_mime("video/webm"),
    }
}

fn file_name_with_extension(path: &Path, extension: &str) -> String {
    let stem = path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("attachment");
    if extension.is_empty() {
        return path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("attachment")
            .to_owned();
    }

    format!("{stem}.{extension}")
}

async fn prepare_image_upload(
    app_handle: &AppHandle,
    room_id_raw: &str,
    path: &Path,
    bytes: &[u8],
    compress_media: bool,
    cancellation_flag: Arc<AtomicBool>,
) -> Result<PreparedUpload, String> {
    let is_gif = path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gif"));

    if !compress_media {
        return Ok(PreparedUpload {
            bytes: bytes.to_vec(),
            content_type: guess_image_mime(path)?,
            file_name: file_name_with_extension(path, ""),
            transcode_mode: VideoTranscodeMode::Software,
        });
    }

    if is_gif {
        return transcode_gif_to_webp(app_handle, room_id_raw, path, cancellation_flag).await;
    }

    let decoded = image::load_from_memory(bytes)
        .map_err(|error| format!("Failed to decode image: {error}"))?;
    let mut output = std::io::Cursor::new(Vec::new());
    decoded
        .write_to(&mut output, image::ImageFormat::WebP)
        .map_err(|error| format!("Failed to encode WebP image: {error}"))?;

    Ok(PreparedUpload {
        bytes: output.into_inner(),
        content_type: parse_mime("image/webp")?,
        file_name: file_name_with_extension(path, "webp"),
        transcode_mode: VideoTranscodeMode::Software,
    })
}

async fn transcode_gif_to_webp(
    app_handle: &AppHandle,
    room_id_raw: &str,
    path: &Path,
    cancellation_flag: Arc<AtomicBool>,
) -> Result<PreparedUpload, String> {
    let input_path = path.to_path_buf();
    let output_path = temp_output_path(path, "webp");
    let mode = VideoTranscodeMode::Software;

    emit_transcode_progress(app_handle, room_id_raw, path, "transcoding", 0.0, mode)?;

    let pipeline = build_image_transcode_pipeline(&input_path, &output_path, true, mode);

    run_gstreamer_pipeline_with_progress(
        &pipeline,
        "animated WebP GIF",
        app_handle,
        room_id_raw,
        path,
        mode,
        cancellation_flag,
    )
    .await?;

    let bytes = std::fs::read(&output_path)
        .map_err(|error| format!("Failed to read converted animated WebP: {error}"))?;

    emit_transcode_progress(app_handle, room_id_raw, path, "finalizing", 100.0, mode)?;

    Ok(PreparedUpload {
        bytes,
        content_type: parse_mime("image/webp")?,
        file_name: file_name_with_extension(path, "webp"),
        transcode_mode: mode,
    })
}

async fn prepare_video_upload(
    app_handle: &AppHandle,
    room_id_raw: &str,
    path: &Path,
    compress_media: bool,
    cancellation_flag: Arc<AtomicBool>,
) -> Result<PreparedUpload, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| format!("Failed to read video file: {error}"))?;

    if !compress_media {
        return Ok(PreparedUpload {
            bytes,
            content_type: guess_video_mime(path)?,
            file_name: file_name_with_extension(path, ""),
            transcode_mode: VideoTranscodeMode::Software,
        });
    }

    let input_path = path.to_path_buf();
    let plan = detect_video_transcode_plan();
    let (extension, content_type, description) = match plan.codec {
        VideoCodec::H264 => ("mp4", parse_mime("video/mp4")?, "H264 MP4 video"),
        VideoCodec::Vp8 => ("webm", parse_mime("video/webm")?, "VP8 WebM video"),
        VideoCodec::Vp9 => ("webm", parse_mime("video/webm")?, "VP9 WebM video"),
    };

    let output_path = temp_output_path(path, extension);

    emit_transcode_progress(app_handle, room_id_raw, path, "transcoding", 0.0, plan.mode)?;

    let pipeline = build_video_transcode_pipeline(&input_path, &output_path, plan);

    run_gstreamer_pipeline_with_progress(
        &pipeline,
        description,
        app_handle,
        room_id_raw,
        path,
        plan.mode,
        cancellation_flag,
    )
    .await?;

    let bytes = std::fs::read(&output_path)
        .map_err(|error| format!("Failed to read converted video: {error}"))?;

    emit_transcode_progress(
        app_handle,
        room_id_raw,
        path,
        "finalizing",
        100.0,
        plan.mode,
    )?;

    Ok(PreparedUpload {
        bytes,
        content_type,
        file_name: file_name_with_extension(path, extension),
        transcode_mode: plan.mode,
    })
}

fn temp_output_path(path: &Path, extension: &str) -> PathBuf {
    let mut output = media_cache_dir_path();
    output.push("transcode");

    if std::fs::create_dir_all(&output).is_err() {
        output = std::env::temp_dir();
        output.push("singularity");
        output.push("media-cache");
        output.push("transcode");
        let _ = std::fs::create_dir_all(&output);
    }

    let stem = path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("attachment");
    let nonce = rand::random::<u64>();
    output.push(format!("singularity-{stem}-{nonce}.{extension}"));
    output
}

fn detect_video_transcode_plan() -> VideoTranscodePlan {
    if gst_element_available("vaapivp9enc") {
        return VideoTranscodePlan {
            mode: VideoTranscodeMode::Vaapi,
            codec: VideoCodec::Vp9,
        };
    }

    if gst_element_available("vaapivp8enc") {
        return VideoTranscodePlan {
            mode: VideoTranscodeMode::Vaapi,
            codec: VideoCodec::Vp8,
        };
    }

    if gst_element_available("vaapih264enc") {
        return VideoTranscodePlan {
            mode: VideoTranscodeMode::Vaapi,
            codec: VideoCodec::H264,
        };
    }

    if gst_element_available("nvvp9enc") {
        return VideoTranscodePlan {
            mode: VideoTranscodeMode::Cuda,
            codec: VideoCodec::Vp9,
        };
    }

    if gst_element_available("nvvp8enc") {
        return VideoTranscodePlan {
            mode: VideoTranscodeMode::Cuda,
            codec: VideoCodec::Vp8,
        };
    }

    if gst_element_available("nvh264enc") {
        return VideoTranscodePlan {
            mode: VideoTranscodeMode::Cuda,
            codec: VideoCodec::H264,
        };
    }

    VideoTranscodePlan {
        mode: VideoTranscodeMode::Software,
        codec: VideoCodec::Vp9,
    }
}

fn gst_element_available(element_name: &str) -> bool {
    Command::new("gst-inspect-1.0")
        .arg(element_name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn build_video_transcode_pipeline(
    input_path: &Path,
    output_path: &Path,
    plan: VideoTranscodePlan,
) -> Vec<String> {
    let mux_element = match plan.codec {
        VideoCodec::H264 => String::from("mp4mux"),
        VideoCodec::Vp8 | VideoCodec::Vp9 => String::from("webmmux"),
    };

    let mut pipeline = vec![
        String::from("-q"),
        String::from("-e"),
        String::from("filesrc"),
        format!("location={}", input_path.to_string_lossy()),
        String::from("!"),
        String::from("decodebin"),
        String::from("name=dec"),
        mux_element,
        String::from("faststart=true"),
        String::from("name=mux"),
        String::from("!"),
        String::from("filesink"),
        format!("location={}", output_path.to_string_lossy()),
        String::from("dec."),
        String::from("!"),
        String::from("queue"),
        String::from("!"),
    ];

    match plan.mode {
        VideoTranscodeMode::Vaapi => {
            if gst_element_available("vaapipostproc") {
                pipeline.push(String::from("vaapipostproc"));
                pipeline.push(String::from("!"));
            } else {
                pipeline.push(String::from("videoconvert"));
                pipeline.push(String::from("!"));
            }
            match plan.codec {
                VideoCodec::Vp9 => pipeline.push(String::from("vaapivp9enc")),
                VideoCodec::Vp8 => pipeline.push(String::from("vaapivp8enc")),
                VideoCodec::H264 => pipeline.push(String::from("vaapih264enc")),
            }
        }
        VideoTranscodeMode::Cuda => {
            pipeline.push(String::from("videoconvert"));
            pipeline.push(String::from("!"));
            if gst_element_available("cudaconvert") {
                pipeline.push(String::from("cudaconvert"));
                pipeline.push(String::from("!"));
            }
            match plan.codec {
                VideoCodec::Vp9 => pipeline.push(String::from("nvvp9enc")),
                VideoCodec::Vp8 => pipeline.push(String::from("nvvp8enc")),
                VideoCodec::H264 => pipeline.push(String::from("nvh264enc")),
            }
        }
        VideoTranscodeMode::Software => {
            pipeline.push(String::from("videoconvert"));
            pipeline.push(String::from("!"));
            pipeline.push(String::from("vp9enc"));
            pipeline.push(String::from("deadline=1"));
        }
    }

    if matches!(plan.codec, VideoCodec::H264) {
        pipeline.extend([
            String::from("!"),
            String::from("h264parse"),
            String::from("config-interval=-1"),
            String::from("!"),
            String::from("video/x-h264,stream-format=avc,alignment=au"),
        ]);
    }

    pipeline.extend([
        String::from("!"),
        String::from("progressreport"),
        String::from("update-freq=1"),
        String::from("!"),
        String::from("mux."),
        String::from("dec."),
        String::from("!"),
        String::from("queue"),
        String::from("!"),
        String::from("audioconvert"),
        String::from("!"),
        String::from("audioresample"),
        String::from("!"),
    ]);

    match plan.codec {
        VideoCodec::H264 => {
            pipeline.push(String::from("avenc_aac"));
            pipeline.push(String::from("!"));
            pipeline.push(String::from("aacparse"));
            pipeline.push(String::from("!"));
        }
        VideoCodec::Vp8 | VideoCodec::Vp9 => {
            pipeline.push(String::from("opusenc"));
            pipeline.push(String::from("bitrate=128000"));
            pipeline.push(String::from("!"));
        }
    }

    pipeline.push(String::from("mux."));

    pipeline
}

fn build_image_transcode_pipeline(
    input_path: &Path,
    output_path: &Path,
    animated: bool,
    mode: VideoTranscodeMode,
) -> Vec<String> {
    let mut pipeline = vec![
        String::from("-q"),
        String::from("-e"),
        String::from("filesrc"),
        format!("location={}", input_path.to_string_lossy()),
        String::from("!"),
    ];

    match mode {
        VideoTranscodeMode::Vaapi => {
            if gst_element_available("vaapidecodebin") {
                pipeline.push(String::from("vaapidecodebin"));
            } else {
                pipeline.push(String::from("decodebin"));
            }
        }
        VideoTranscodeMode::Cuda => {
            pipeline.push(String::from("decodebin"));
        }
        VideoTranscodeMode::Software => {
            pipeline.push(String::from("decodebin"));
        }
    }

    pipeline.extend([
        String::from("!"),
        String::from("videoconvert"),
        String::from("!"),
        String::from("progressreport"),
        String::from("update-freq=1"),
        String::from("!"),
        String::from("webpenc"),
    ]);

    if animated {
        pipeline.push(String::from("animated=true"));
    }

    pipeline.extend([
        String::from("quality=80"),
        String::from("!"),
        String::from("filesink"),
        format!("location={}", output_path.to_string_lossy()),
    ]);

    pipeline
}

async fn run_gstreamer_pipeline_with_progress(
    tokens: &[String],
    description: &str,
    app_handle: &AppHandle,
    room_id_raw: &str,
    file_path: &Path,
    mode: VideoTranscodeMode,
    cancellation_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    if cancellation_flag.load(Ordering::Relaxed) {
        let _ = emit_transcode_progress(app_handle, room_id_raw, file_path, "cancelled", 0.0, mode);
        return Err(String::from("Transcode cancelled by user"));
    }

    let mut command = TokioCommand::new("gst-launch-1.0");
    command
        .args(tokens)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to run GStreamer for {description}: {error}"))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("Failed to capture GStreamer stdout for {description}"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("Failed to capture GStreamer stderr for {description}"))?;

    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    let stdout_tx = line_tx.clone();
    let stderr_tx = line_tx.clone();

    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(&mut stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = stdout_tx.send(line);
        }
    });

    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(&mut stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = stderr_tx.send(line);
        }
    });

    let progress_app_handle = app_handle.clone();
    let room_id = room_id_raw.to_owned();
    let file_path = file_path.to_path_buf();
    let progress_file_path = file_path.clone();
    let progress_task = tokio::spawn(async move {
        while let Some(line) = line_rx.recv().await {
            if let Some(progress) = parse_progressreport_line(line.as_str()) {
                let _ = emit_transcode_progress(
                    &progress_app_handle,
                    room_id.as_str(),
                    progress_file_path.as_path(),
                    "transcoding",
                    progress,
                    mode,
                );
            }
        }
    });

    let status = loop {
        if cancellation_flag.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            let _ = child.wait().await;
            break None;
        }

        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                tokio::time::sleep(Duration::from_millis(125)).await;
            }
            Err(error) => {
                return Err(format!(
                    "Failed to wait for GStreamer process for {description}: {error}"
                ));
            }
        }
    };

    let _ = stdout_task.await;
    let _ = stderr_task.await;
    drop(line_tx);
    let _ = progress_task.await;

    if status.is_none() {
        let _ =
            emit_transcode_progress(app_handle, room_id_raw, &file_path, "cancelled", 0.0, mode);
        return Err(String::from("Transcode cancelled by user"));
    }

    if !status.is_some_and(|value| value.success()) {
        return Err(format!("GStreamer failed to convert {description}"));
    }

    Ok(())
}

fn parse_progressreport_line(line: &str) -> Option<f64> {
    let percent_start = line.rfind('(')?;
    let percent_slice = line.get(percent_start + 1..)?.split('%').next()?.trim();
    let value = percent_slice.replace(',', ".");
    value.parse::<f64>().ok()
}

fn transmission_progress_percent(progress: TransmissionProgress) -> f64 {
    if progress.total == 0 {
        return 0.0;
    }

    (progress.current as f64 / progress.total as f64 * 100.0).clamp(0.0, 100.0)
}

fn emit_transcode_progress(
    app_handle: &AppHandle,
    room_id_raw: &str,
    file_path: &Path,
    stage: &str,
    progress: f64,
    mode: VideoTranscodeMode,
) -> Result<(), String> {
    let event = MatrixMediaTranscodeProgressEvent {
        room_id: room_id_raw.to_owned(),
        file_path: file_path.to_string_lossy().to_string(),
        stage: stage.to_owned(),
        progress: progress.clamp(0.0, 100.0),
        hardware_mode: match mode {
            VideoTranscodeMode::Vaapi => String::from("vaapi"),
            VideoTranscodeMode::Cuda => String::from("cuda"),
            VideoTranscodeMode::Software => String::from("software"),
        },
    };

    app_handle
        .emit(event_paths::MEDIA_TRANSCODE_PROGRESS, event)
        .map_err(|error| format!("Failed to emit transcode progress: {error}"))
}
