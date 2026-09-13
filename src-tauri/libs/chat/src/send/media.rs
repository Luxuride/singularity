use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use matrix_sdk::TransmissionProgress;
use mime::Mime;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::mpsc;

use assets::media_cache_dir_path;
use types::chat::MatrixMediaTranscodeProgressEvent;
use types::event_paths;
use types::EventSink;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoTranscodeMode {
    Vaapi,
    Cuda,
    Software,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    Vp9,
    Vp8,
    H264,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoTranscodePlan {
    pub mode: VideoTranscodeMode,
    pub codec: VideoCodec,
}

#[derive(Clone, Debug)]
pub enum MediaKind {
    Image,
    Video,
    File,
}

#[derive(Clone)]
pub struct PreparedUpload {
    pub bytes: Vec<u8>,
    pub content_type: Mime,
    pub file_name: String,
    pub transcode_mode: VideoTranscodeMode,
}

pub fn detect_media_kind(path: &Path, bytes: &[u8]) -> MediaKind {
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

pub fn parse_mime(raw: &str) -> Result<Mime, String> {
    raw.parse::<Mime>()
        .map_err(|error| format!("Invalid mime type {raw}: {error}"))
}

pub fn guess_image_mime(path: &Path) -> Result<Mime, String> {
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

pub fn guess_video_mime(path: &Path) -> Result<Mime, String> {
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

pub fn file_name_with_extension(path: &Path, extension: &str) -> String {
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

pub fn temp_output_path(path: &Path, extension: &str) -> PathBuf {
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

pub fn detect_video_transcode_plan() -> VideoTranscodePlan {
    if gst_element_available("vavp9enc") {
        return VideoTranscodePlan {
            mode: VideoTranscodeMode::Vaapi,
            codec: VideoCodec::Vp9,
        };
    }

    if gst_element_available("vavp8enc") {
        return VideoTranscodePlan {
            mode: VideoTranscodeMode::Vaapi,
            codec: VideoCodec::Vp8,
        };
    }

    if gst_element_available("vah264enc") {
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

pub fn gst_element_available(element_name: &str) -> bool {
    Command::new("gst-inspect-1.0")
        .arg(element_name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn build_video_transcode_pipeline(
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
            if gst_element_available("vapostproc") {
                pipeline.push(String::from("vapostproc"));
                pipeline.push(String::from("!"));
            } else {
                pipeline.push(String::from("videoconvert"));
                pipeline.push(String::from("!"));
            }
            match plan.codec {
                VideoCodec::Vp9 => pipeline.push(String::from("vavp9enc")),
                VideoCodec::Vp8 => pipeline.push(String::from("vavp8enc")),
                VideoCodec::H264 => pipeline.push(String::from("vah264enc")),
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

pub fn build_image_transcode_pipeline(
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
            if gst_element_available("vadecodebin") {
                pipeline.push(String::from("vadecodebin"));
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

pub async fn run_gstreamer_pipeline_with_progress(
    tokens: &[String],
    description: &str,
    event_sink: &Arc<dyn EventSink>,
    room_id_raw: &str,
    file_path: &Path,
    mode: VideoTranscodeMode,
    cancellation_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    if cancellation_flag.load(Ordering::Relaxed) {
        let _ = emit_transcode_progress(event_sink, room_id_raw, file_path, "cancelled", 0.0, mode);
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

    let event_sink_for_progress = event_sink.clone();
    let room_id = room_id_raw.to_owned();
    let file_path_buf = file_path.to_path_buf();
    let progress_file_path = file_path_buf.clone();
    let progress_task = tokio::spawn(async move {
        while let Some(line) = line_rx.recv().await {
            if let Some(progress) = parse_progressreport_line(line.as_str()) {
                let _ = emit_transcode_progress(
                    &event_sink_for_progress,
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
        let _ = emit_transcode_progress(
            event_sink,
            room_id_raw,
            &file_path_buf,
            "cancelled",
            0.0,
            mode,
        );
        return Err(String::from("Transcode cancelled by user"));
    }

    if !status.is_some_and(|value| value.success()) {
        return Err(format!("GStreamer failed to convert {description}"));
    }

    Ok(())
}

pub fn parse_progressreport_line(line: &str) -> Option<f64> {
    let percent_start = line.rfind('(')?;
    let percent_slice = line.get(percent_start + 1..)?.split('%').next()?.trim();
    let value = percent_slice.replace(',', ".");
    value.parse::<f64>().ok()
}

pub fn transmission_progress_percent(progress: TransmissionProgress) -> f64 {
    if progress.total == 0 {
        return 0.0;
    }

    (progress.current as f64 / progress.total as f64 * 100.0).clamp(0.0, 100.0)
}

pub fn emit_transcode_progress(
    event_sink: &Arc<dyn EventSink>,
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

    let payload = serde_json::to_value(&event)
        .map_err(|error| format!("Failed to serialize transcode progress: {error}"))?;
    event_sink
        .emit(event_paths::MEDIA_TRANSCODE_PROGRESS, &payload)
        .map_err(|error| format!("Failed to emit transcode progress: {error}"))
}

pub async fn prepare_image_upload(
    event_sink: &Arc<dyn EventSink>,
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
        return transcode_gif_to_webp(event_sink, room_id_raw, path, cancellation_flag).await;
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
    event_sink: &Arc<dyn EventSink>,
    room_id_raw: &str,
    path: &Path,
    cancellation_flag: Arc<AtomicBool>,
) -> Result<PreparedUpload, String> {
    let input_path = path.to_path_buf();
    let output_path = temp_output_path(path, "webp");
    let mode = VideoTranscodeMode::Software;

    emit_transcode_progress(event_sink, room_id_raw, path, "transcoding", 0.0, mode)?;

    let pipeline = build_image_transcode_pipeline(&input_path, &output_path, true, mode);

    run_gstreamer_pipeline_with_progress(
        &pipeline,
        "animated WebP GIF",
        event_sink,
        room_id_raw,
        path,
        mode,
        cancellation_flag,
    )
    .await?;

    let bytes = std::fs::read(&output_path)
        .map_err(|error| format!("Failed to read converted animated WebP: {error}"))?;

    emit_transcode_progress(event_sink, room_id_raw, path, "finalizing", 100.0, mode)?;

    Ok(PreparedUpload {
        bytes,
        content_type: parse_mime("image/webp")?,
        file_name: file_name_with_extension(path, "webp"),
        transcode_mode: mode,
    })
}

pub async fn prepare_video_upload(
    event_sink: &Arc<dyn EventSink>,
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

    emit_transcode_progress(event_sink, room_id_raw, path, "transcoding", 0.0, plan.mode)?;

    let pipeline = build_video_transcode_pipeline(&input_path, &output_path, plan);

    run_gstreamer_pipeline_with_progress(
        &pipeline,
        description,
        event_sink,
        room_id_raw,
        path,
        plan.mode,
        cancellation_flag,
    )
    .await?;

    let bytes = std::fs::read(&output_path)
        .map_err(|error| format!("Failed to read converted video: {error}"))?;

    emit_transcode_progress(
        event_sink,
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
