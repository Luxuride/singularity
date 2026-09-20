//! Generic file downloads: saving file message bytes to disk.
//!
//! Unlike images and videos, generic `m.file` messages are **not cached** on
//! disk. Files are downloaded on demand and written directly to a user-chosen
//! destination path (via a native save dialog in the command layer). This
//! module provides the disk-write primitive and MIME <-> extension mapping.
//!
//! The implementation is split by concern:
//!
//! - `mime` — MIME type <-> extension mapping for default file names.

pub mod mime;

use std::fs;
use std::path::Path;

pub use mime::file_extension_from_mime;

/// Write downloaded file bytes to a destination path on disk. The parent
/// directory must already exist (the caller chooses the path via a save
/// dialog). Returns an error message on failure.
pub fn save_file_to_path(bytes: &[u8], destination: &Path) -> Result<(), String> {
    fs::write(destination, bytes)
        .map_err(|error| format!("Failed to write file to {}: {error}", destination.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{file_extension_from_mime, save_file_to_path};

    #[test]
    fn file_extension_from_mime_maps_common_types() {
        assert_eq!(file_extension_from_mime("application/pdf"), "pdf");
        assert_eq!(file_extension_from_mime("application/zip"), "zip");
        assert_eq!(file_extension_from_mime("text/plain"), "txt");
        assert_eq!(file_extension_from_mime("application/unknown"), "bin");
    }

    #[test]
    fn save_file_to_path_writes_verbatim_bytes() {
        let dir = std::env::temp_dir().join("singularity-test-file-download");
        fs::create_dir_all(&dir).expect("create temp download dir");
        let file = dir.join("download.bin");

        save_file_to_path(&[1, 2, 3], &file).expect("save file");
        assert_eq!(fs::read(&file).expect("read saved file"), vec![1, 2, 3]);

        fs::remove_file(&file).expect("clean up temp file");
        fs::remove_dir(&dir).expect("clean up temp dir");
    }
}
