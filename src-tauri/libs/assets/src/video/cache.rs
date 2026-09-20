//! Video caching: local disk-backed cache for video messages.
//!
//! Videos are cached verbatim (no re-encoding) so the original container and
//! codec are preserved. The cache key is derived from the event's media source
//! and byte length, and the payload is persisted through the shared
//! `image::persistence` module.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::cache_key::VideoCacheKeyParts;
use super::mime::video_extension_from_mime;
use crate::image::persistence::{persist_normalized_media, NormalizedMediaLoad};

/// Cache video bytes to the media cache directory and return the resolved
/// `asset://` URL. Bytes are written verbatim to preserve the original
/// container/codec.
pub fn cache_video(bytes: &[u8], key_parts: VideoCacheKeyParts) -> Option<String> {
    let extension = video_extension_from_mime(&key_parts.mime_type);
    let file_stem = video_cache_key(&key_parts);

    let request = NormalizedMediaLoad::builder()
        .bytes(bytes)
        .file_stem(file_stem)
        .extension(extension)
        .mime_type(key_parts.mime_type)
        .build()?;

    persist_normalized_media(&request)
}

fn video_cache_key(parts: &VideoCacheKeyParts) -> String {
    let mut hasher = DefaultHasher::new();
    parts.source_key.hash(&mut hasher);
    parts.mime_type.hash(&mut hasher);
    parts.bytes_len.hash(&mut hasher);
    format!("vid-{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::{cache_video, VideoCacheKeyParts};

    #[test]
    fn cache_video_persists_verbatim_bytes() {
        let key_parts = VideoCacheKeyParts::builder()
            .source_key(Some("mxc://server/media"))
            .mime_type("video/mp4")
            .bytes_len(3)
            .build()
            .expect("complete key parts");

        let url = cache_video(&[1, 2, 3], key_parts).expect("cache video");
        assert!(url.starts_with("asset://localhost/"));
        assert_eq!(
            crate::image::persistence::load_media_bytes_from_resolved_url(&url),
            Some(vec![1, 2, 3]),
        );
    }
}
