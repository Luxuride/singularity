/// MIME type <-> file extension mapping for cached media. Image, video, audio,
/// and generic file types are all mapped here so the cache can derive a stable
/// extension from a MIME type (and vice versa).
pub fn image_extension_from_mime(mime_type: &str) -> &'static str {
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

pub fn media_extension_from_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/ogg" => "ogv",
        "video/quicktime" => "mov",
        "video/x-matroska" => "mkv",
        "video/mpeg" => "mpeg",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/opus" => "opus",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/flac" => "flac",
        "audio/aac" => "aac",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "text/plain" => "txt",
        _ => image_extension_from_mime(mime_type),
    }
}

pub(super) fn mime_type_from_extension(extension: &str) -> &'static str {
    match extension {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

pub(super) fn image_extension_from_raw_url(raw_url: &str) -> &'static str {
    let file_name = raw_url
        .trim_start_matches("mxc://")
        .rsplit('/')
        .next()
        .unwrap_or_default();

    let extension = file_name.rsplit('.').next().unwrap_or_default();

    match extension.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "jpg",
        "png" => "png",
        "gif" => "gif",
        "webp" => "webp",
        "avif" => "avif",
        "bmp" => "bmp",
        "svg" => "svg",
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::{image_extension_from_mime, media_extension_from_mime};

    #[test]
    fn image_extension_from_mime_is_stable() {
        assert_eq!(image_extension_from_mime("image/jpeg"), "jpg");
        assert_eq!(image_extension_from_mime("image/png"), "png");
        assert_eq!(image_extension_from_mime("image/unknown"), "bin");
    }

    #[test]
    fn media_extension_from_mime_maps_video_and_audio() {
        assert_eq!(media_extension_from_mime("video/mp4"), "mp4");
        assert_eq!(media_extension_from_mime("video/webm"), "webm");
        assert_eq!(media_extension_from_mime("audio/mpeg"), "mp3");
        assert_eq!(media_extension_from_mime("audio/opus"), "opus");
        // Falls back to image extension mapping for image mime types.
        assert_eq!(media_extension_from_mime("image/png"), "png");
        // Unknown mime types fall back to the generic binary extension.
        assert_eq!(media_extension_from_mime("application/octet-stream"), "bin");
    }
}
