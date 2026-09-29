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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Extensions treated as video when the sniffed bytes are not an image.
///
/// This list is the single source of truth: it is shared by the kind detection
/// above and the mime guess below, which previously enumerated video extensions
/// independently and disagreed — `detect_media_kind` routed `.mkv` and `.avi`
/// to `MediaKind::Video`, but `guess_video_mime` had no arm for either and its
/// catch-all labelled them `video/webm`. The homeserver stores that content type
/// on the mxc object and stamps it into the event, so a Matroska file reached the
/// recipient's player with a WebM header and failed to render.
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "mov", "webm", "avi"];

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
        Some(extension) if VIDEO_EXTENSIONS.contains(&extension) => MediaKind::Video,
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
        Some("mp4") => parse_mime("video/mp4"),
        Some("mov") => parse_mime("video/quicktime"),
        Some("mkv") => parse_mime("video/x-matroska"),
        Some("avi") => parse_mime("video/x-msvideo"),
        // `webm` and anything unrecognised. An unknown container is only ever
        // reached here after [`detect_media_kind`] classified it as video, and
        // webm is the container this app's transcode produces.
        _ => parse_mime("video/webm"),
    }
}

/// Content type for a plain file attachment, from its extension.
///
/// The `MediaKind::File` arm used to send every document as
/// `application/octet-stream` even though the real extension was in hand, so a
/// `report.pdf` reached receivers that render from `info.mimetype` as an
/// unopenable blob. `application/octet-stream` stays the fallback for a name
/// with no usable extension.
pub fn guess_mime_from_extension(file_name: &str) -> Result<Mime, String> {
    let extension = Path::new(file_name)
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.to_ascii_lowercase());

    let mime = match extension.as_deref() {
        Some("pdf") => "application/pdf",
        Some("txt") | Some("md") | Some("log") => "text/plain",
        Some("json") => "application/json",
        Some("zip") => "application/zip",
        Some("gz") => "application/gzip",
        Some("epub") => "application/epub+zip",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xls") => "application/vnd.ms-excel",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("opus") => "audio/opus",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        _ => "application/octet-stream",
    };

    parse_mime(mime)
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

/// Deletes a transcode temp file created by [`temp_output_path`].
///
/// A transcode writes a full-size copy of the upload to disk, so leaving it
/// behind leaks a copy per compressed upload until the next app launch (the
/// only other cleanup is `clear_media_cache` at startup). Best-effort: the
/// upload result must not depend on the cleanup succeeding.
pub fn remove_temp_output(output_path: &Path) {
    if let Err(error) = std::fs::remove_file(output_path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!("Failed to remove transcode temp file {output_path:?}: {error}");
        }
    }
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
    // `faststart` is a property of mp4mux only; webmmux has no such property, so
    // emitting it there makes gst_parse_launch reject the whole pipeline.
    let (mux_element, supports_faststart) = match plan.codec {
        VideoCodec::H264 => (String::from("mp4mux"), true),
        VideoCodec::Vp8 | VideoCodec::Vp9 => (String::from("webmmux"), false),
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
    ];
    if supports_faststart {
        pipeline.push(String::from("faststart=true"));
    }
    pipeline.extend([
        String::from("name=mux"),
        String::from("!"),
        String::from("filesink"),
        format!("location={}", output_path.to_string_lossy()),
        String::from("dec."),
        String::from("!"),
        String::from("queue"),
        String::from("!"),
    ]);

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

/// Wall-clock ceiling for an animated GIF to WebP transcode.
///
/// A transcode that outlives this is wedged, not slow: the hardware plans are
/// chosen by `gst-inspect-1.0` finding an element, without proving the device
/// actually works, so a busy or wedged GPU is a live path.
const GIF_TRANSCODE_TIMEOUT: Duration = Duration::from_secs(300);

/// Wall-clock ceiling for a video transcode. See [`GIF_TRANSCODE_TIMEOUT`].
const VIDEO_TRANSCODE_TIMEOUT: Duration = Duration::from_secs(1_800);

#[allow(clippy::too_many_arguments)]
pub async fn run_gstreamer_pipeline_with_progress(
    tokens: &[String],
    description: &str,
    event_sink: &Arc<dyn EventSink>,
    room_id_raw: &str,
    file_path: &Path,
    mode: VideoTranscodeMode,
    cancellation_flag: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<(), String> {
    if cancellation_flag.load(Ordering::Relaxed) {
        let _ = emit_transcode_progress(event_sink, room_id_raw, file_path, "cancelled", 0.0, mode);
        return Err(String::from("Transcode cancelled by user"));
    }

    let deadline = tokio::time::Instant::now() + timeout;

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

    // The poll loop below has to bound how long a wedged GStreamer can run, and
    // every exit has to go through the same cleanup. A child that outlives its
    // own send keeps burning the GPU and holds its output file open, so neither
    // the deadline nor a `try_wait` failure may skip the kill/await.
    let status = loop {
        if cancellation_flag.load(Ordering::Relaxed) {
            let _ = child.kill().await;
            let _ = child.wait().await;
            break PipelineOutcome::Cancelled;
        }

        if tokio::time::Instant::now() >= deadline {
            let _ = child.kill().await;
            let _ = child.wait().await;
            break PipelineOutcome::TimedOut;
        }

        match child.try_wait() {
            Ok(Some(status)) => break PipelineOutcome::Finished(status),
            Ok(None) => {
                tokio::time::sleep(Duration::from_millis(125)).await;
            }
            Err(error) => {
                // Killing a process that is already gone is not an error here:
                // the point is that no task and no process is left running.
                let _ = child.kill().await;
                let _ = child.wait().await;
                break PipelineOutcome::Failed(format!(
                    "Failed to wait for GStreamer process for {description}: {error}"
                ));
            }
        }
    };

    let _ = stdout_task.await;
    let _ = stderr_task.await;
    drop(line_tx);
    let _ = progress_task.await;

    match status {
        PipelineOutcome::Cancelled => {
            let _ = emit_transcode_progress(
                event_sink,
                room_id_raw,
                &file_path_buf,
                "cancelled",
                0.0,
                mode,
            );
            Err(String::from("Transcode cancelled by user"))
        }
        PipelineOutcome::TimedOut => Err(format!(
            "Timed out after {}s waiting for GStreamer to convert {description}",
            timeout.as_secs()
        )),
        PipelineOutcome::Failed(error) => Err(error),
        PipelineOutcome::Finished(status) if status.success() => Ok(()),
        PipelineOutcome::Finished(_) => Err(format!("GStreamer failed to convert {description}")),
    }
}

/// How a [`run_gstreamer_pipeline_with_progress`] poll loop ended.
enum PipelineOutcome {
    Finished(std::process::ExitStatus),
    Cancelled,
    TimedOut,
    Failed(String),
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

/// Report transcode progress without letting the report itself fail the send.
///
/// Progress is a UI notification, so a serialisation or event-bus failure must
/// never decide the outcome of a media send. The data path used to propagate
/// this with `?`, which turned a finished transcode into an error whenever a
/// listener window closed, while the per-line progress loop in the same
/// functions ignored the identical failure with `let _ =`. Both paths now log
/// and continue.
pub fn report_transcode_progress(
    event_sink: &Arc<dyn EventSink>,
    room_id_raw: &str,
    file_path: &Path,
    stage: &str,
    progress: f64,
    mode: VideoTranscodeMode,
) {
    if let Err(error) =
        emit_transcode_progress(event_sink, room_id_raw, file_path, stage, progress, mode)
    {
        log::warn!("Failed to report transcode progress ({stage}): {error}");
    }
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

    report_transcode_progress(event_sink, room_id_raw, path, "transcoding", 0.0, mode);

    let pipeline = build_image_transcode_pipeline(&input_path, &output_path, true, mode);

    if let Err(error) = run_gstreamer_pipeline_with_progress(
        &pipeline,
        "animated WebP GIF",
        event_sink,
        room_id_raw,
        path,
        mode,
        cancellation_flag,
        GIF_TRANSCODE_TIMEOUT,
    )
    .await
    {
        remove_temp_output(&output_path);
        return Err(error);
    }

    let bytes = match std::fs::read(&output_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            remove_temp_output(&output_path);
            return Err(format!("Failed to read converted animated WebP: {error}"));
        }
    };
    remove_temp_output(&output_path);

    report_transcode_progress(event_sink, room_id_raw, path, "finalizing", 100.0, mode);

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
    // Read the source only on the pass-through branch. The transcode branch
    // lets GStreamer read the file itself and returns the transcoded output, so
    // materialising the whole source here would add a full extra copy of the
    // file to peak memory for nothing.
    if !compress_media {
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|error| format!("Failed to read video file: {error}"))?;

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

    report_transcode_progress(event_sink, room_id_raw, path, "transcoding", 0.0, plan.mode);

    let pipeline = build_video_transcode_pipeline(&input_path, &output_path, plan);

    if let Err(error) = run_gstreamer_pipeline_with_progress(
        &pipeline,
        description,
        event_sink,
        room_id_raw,
        path,
        plan.mode,
        cancellation_flag,
        VIDEO_TRANSCODE_TIMEOUT,
    )
    .await
    {
        remove_temp_output(&output_path);
        return Err(error);
    }

    let bytes = match std::fs::read(&output_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            remove_temp_output(&output_path);
            return Err(format!("Failed to read converted video: {error}"));
        }
    };
    remove_temp_output(&output_path);

    report_transcode_progress(
        event_sink,
        room_id_raw,
        path,
        "finalizing",
        100.0,
        plan.mode,
    );

    Ok(PreparedUpload {
        bytes,
        content_type,
        file_name: file_name_with_extension(path, extension),
        transcode_mode: plan.mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: `guess_video_mime` and `detect_media_kind` used to enumerate
    /// video extensions independently and disagreed, so a Matroska or AVI file
    /// was uploaded to the homeserver as `video/webm`. The recipient's player
    /// then received a WebM header for non-WebM bytes and refused to render it.
    #[test]
    fn every_video_extension_gets_its_own_mime() {
        let cases = [
            ("clip.mp4", "video/mp4"),
            ("clip.mov", "video/quicktime"),
            ("clip.mkv", "video/x-matroska"),
            ("clip.avi", "video/x-msvideo"),
            ("clip.webm", "video/webm"),
        ];

        for (file_name, expected) in cases {
            let path = Path::new(file_name);
            // A file whose bytes are not an image reaches the extension rule.
            let kind = detect_media_kind(path, b"not-an-image");
            assert_eq!(kind, MediaKind::Video, "{file_name} should be video");
            assert_eq!(
                guess_video_mime(path).unwrap().essence_str(),
                expected,
                "{file_name}"
            );
        }
    }

    #[test]
    fn image_bytes_win_over_a_video_extension() {
        // The extension list must not override the sniffed content.
        let png_header = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        assert_eq!(
            detect_media_kind(Path::new("clip.mp4"), &png_header),
            MediaKind::Image
        );
    }

    #[test]
    fn unknown_extension_is_a_file_not_a_video() {
        assert_eq!(detect_media_kind(Path::new("notes.txt"), b"hello"), MediaKind::File);
    }

    /// Regression: every document used to be uploaded as
    /// `application/octet-stream`, so a receiver that renders from
    /// `info.mimetype` showed an unopenable blob.
    #[test]
    fn file_attachments_keep_their_real_content_type() {
        for (file_name, expected) in [
            ("report.pdf", "application/pdf"),
            ("notes.txt", "text/plain"),
            ("CHANGELOG.md", "text/plain"),
            ("data.json", "application/json"),
            ("archive.zip", "application/zip"),
            ("deck.pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation"),
            ("song.mp3", "audio/mpeg"),
        ] {
            assert_eq!(
                guess_mime_from_extension(file_name).unwrap().essence_str(),
                expected,
                "{file_name}"
            );
        }
    }

    #[test]
    fn a_name_without_an_extension_falls_back_to_octet_stream() {
        for file_name in ["README", "", "archive.tar."] {
            assert_eq!(
                guess_mime_from_extension(file_name).unwrap().essence_str(),
                "application/octet-stream",
                "{file_name}"
            );
        }
    }
}
