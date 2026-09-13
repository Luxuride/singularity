//! Media and image caching: local media cache, in-memory media store, and the
//! Tauri-free `matrix-media` request body handler.

pub mod image;

pub use image::{
    cache_event_image, cache_mxc_media_to_local_path, canonical_pack_source_url,
    handle_media_request, image_extension_from_mime, initialize_media_cache_dir,
    load_media_bytes_from_resolved_url, media_cache_dir_path, media_storage_mode,
    resolve_pack_media_url, set_media_storage_mode, ImageCacheKeyParts, NormalizedImageLoad,
};
pub use types::media::MediaStorageMode;
