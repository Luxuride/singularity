//! Image and media caching: local disk-backed media cache.
//!
//! This module is a thin facade over the image cache submodules. It re-exports
//! the public API so existing callers can keep using `assets::image::*` paths
//! unchanged. The implementation is split by concern:
//!
//! - `cache_key` — cache key parts used to derive stable file names.
//! - `persistence` — disk-backed cache directory, atomic writes, URL helpers.
//! - `mime` — MIME type <-> extension mapping.
//! - `url` — fetching media from the homeserver and caching it locally.

pub mod cache_key;
pub mod mime;
pub mod persistence;
pub mod url;

pub use cache_key::{ImageCacheKeyParts, ImageCacheKeyPartsBuilder};
pub use mime::{image_extension_from_mime, media_extension_from_mime};
pub use persistence::{
    cached_media_path_for_source_url, clear_media_cache, initialize_media_cache_dir,
    load_media_bytes_from_resolved_url, media_cache_dir_path, media_url_is_available,
    persist_normalized_media, register_cached_media_path, to_asset_storage_url,
    NormalizedMediaLoad, NormalizedMediaLoadBuilder,
};
pub use url::{
    cache_event_image, cache_media_bytes, cache_mxc_media_to_local_path, canonical_pack_source_url,
    resolve_pack_media_url,
};
