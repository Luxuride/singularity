/// MIME type <-> file extension mapping for generic file downloads. Used to
/// derive a sensible default file name extension when saving a downloaded file
/// to disk.
pub fn file_extension_from_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "text/plain" => "txt",
        "application/json" => "json",
        "application/xml" | "text/xml" => "xml",
        "application/octet-stream" => "bin",
        _ => "bin",
    }
}

#[cfg(test)]
mod tests {
    use super::file_extension_from_mime;

    #[test]
    fn file_extension_from_mime_maps_common_types() {
        assert_eq!(file_extension_from_mime("application/pdf"), "pdf");
        assert_eq!(file_extension_from_mime("application/zip"), "zip");
        assert_eq!(file_extension_from_mime("text/plain"), "txt");
        assert_eq!(file_extension_from_mime("application/unknown"), "bin");
    }
}
