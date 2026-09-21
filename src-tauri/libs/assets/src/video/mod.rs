//! Video media caching: local disk-backed cache for video messages.
//!
//! This module is a thin facade over the video cache submodules. It re-exports
//! the public API so existing callers can keep using `assets::video::*` paths
//! unchanged. The implementation is split by concern:
//!
//! - `cache_key` — cache key parts used to derive stable file names.
//! - `mime` — MIME type <-> extension mapping.
//! - `cache` — persisting video bytes to the media cache directory.
//! - `server` — secure loopback HTTP server for serving cached videos.

pub mod cache;
pub mod cache_key;
pub mod mime;
pub mod server;

pub use cache::cache_video;
pub use cache_key::{VideoCacheKeyParts, VideoCacheKeyPartsBuilder};
pub use mime::video_extension_from_mime;
pub use server::VideoServer;
