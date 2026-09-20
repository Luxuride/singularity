//! Media and image caching: local disk-backed media cache, plus generic file
//! download primitives (files are not cached; they are saved on demand).

pub mod file;
pub mod image;
pub mod video;

pub use file::{file_extension_from_mime, save_file_to_path};
pub use image::{
    cache_event_image, cache_media_bytes, cache_mxc_media_to_local_path, canonical_pack_source_url,
    clear_media_cache, image_extension_from_mime, initialize_media_cache_dir,
    load_media_bytes_from_resolved_url, media_cache_dir_path, media_extension_from_mime,
    media_url_is_available, resolve_pack_media_url, ImageCacheKeyParts, NormalizedMediaLoad,
};
pub use video::{
    cache_video, video_extension_from_mime, VideoCacheKeyParts, VideoCacheKeyPartsBuilder,
};
