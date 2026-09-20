/// MIME type <-> file extension mapping for cached video media. Videos are
/// cached verbatim (no re-encoding) so the original container and codec are
/// preserved, and the extension is derived from the MIME type.
pub fn video_extension_from_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/ogg" => "ogv",
        "video/quicktime" => "mov",
        "video/x-matroska" => "mkv",
        "video/mpeg" => "mpeg",
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::video_extension_from_mime;

    #[test]
    fn video_extension_from_mime_maps_common_containers() {
        assert_eq!(video_extension_from_mime("video/mp4"), "mp4");
        assert_eq!(video_extension_from_mime("video/webm"), "webm");
        assert_eq!(video_extension_from_mime("video/quicktime"), "mov");
        assert_eq!(video_extension_from_mime("video/unknown"), "bin");
    }
}
