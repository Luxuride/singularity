use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use log::warn;
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::ruma::events::room::MediaSource;
use percent_encoding::percent_decode_str;

static MEDIA_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
static CACHED_MEDIA_URLS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct ImageCacheKeyParts {
    pub event_id: Option<String>,
    pub origin_server_ts: Option<u64>,
    pub room_id: Option<String>,
    pub source_key: Option<String>,
    pub mime_type: String,
    pub bytes_len: usize,
}

impl ImageCacheKeyParts {
    pub fn builder() -> ImageCacheKeyPartsBuilder {
        ImageCacheKeyPartsBuilder::default()
    }
}

#[derive(Default)]
pub struct ImageCacheKeyPartsBuilder {
    event_id: Option<String>,
    origin_server_ts: Option<u64>,
    room_id: Option<String>,
    source_key: Option<String>,
    mime_type: Option<String>,
    bytes_len: Option<usize>,
}

impl ImageCacheKeyPartsBuilder {
    pub fn event_id<T>(mut self, event_id: Option<T>) -> Self
    where
        T: Into<String>,
    {
        self.event_id = event_id.map(Into::into);
        self
    }

    pub fn origin_server_ts(mut self, origin_server_ts: Option<u64>) -> Self {
        self.origin_server_ts = origin_server_ts;
        self
    }

    pub fn room_id<T>(mut self, room_id: Option<T>) -> Self
    where
        T: Into<String>,
    {
        self.room_id = room_id.map(Into::into);
        self
    }

    pub fn source_key<T>(mut self, source_key: Option<T>) -> Self
    where
        T: Into<String>,
    {
        self.source_key = source_key.map(Into::into);
        self
    }

    pub fn mime_type<T>(mut self, mime_type: T) -> Self
    where
        T: Into<String>,
    {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub fn bytes_len(mut self, bytes_len: usize) -> Self {
        self.bytes_len = Some(bytes_len);
        self
    }

    pub fn build(self) -> Option<ImageCacheKeyParts> {
        Some(ImageCacheKeyParts {
            event_id: self.event_id,
            origin_server_ts: self.origin_server_ts,
            room_id: self.room_id,
            source_key: self.source_key,
            mime_type: self.mime_type?,
            bytes_len: self.bytes_len?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct NormalizedImageLoad {
    pub bytes: Vec<u8>,
    pub file_stem: String,
    pub extension: String,
    pub mime_type: String,
}

impl NormalizedImageLoad {
    pub fn builder() -> NormalizedImageLoadBuilder {
        NormalizedImageLoadBuilder::default()
    }
}

#[derive(Default)]
pub struct NormalizedImageLoadBuilder {
    bytes: Option<Vec<u8>>,
    file_stem: Option<String>,
    extension: Option<String>,
    mime_type: Option<String>,
}

impl NormalizedImageLoadBuilder {
    pub fn bytes(mut self, bytes: &[u8]) -> Self {
        self.bytes = Some(bytes.to_vec());
        self
    }

    pub fn file_stem<T>(mut self, file_stem: T) -> Self
    where
        T: Into<String>,
    {
        self.file_stem = Some(file_stem.into());
        self
    }

    pub fn extension<T>(mut self, extension: T) -> Self
    where
        T: Into<String>,
    {
        self.extension = Some(extension.into());
        self
    }

    pub fn mime_type<T>(mut self, mime_type: T) -> Self
    where
        T: Into<String>,
    {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub fn build(self) -> Option<NormalizedImageLoad> {
        Some(NormalizedImageLoad {
            bytes: self.bytes?,
            file_stem: self.file_stem?,
            extension: self.extension?,
            mime_type: self.mime_type?,
        })
    }
}

pub fn initialize_media_cache_dir(cache_dir: &Path) {
    if MEDIA_CACHE_DIR.get().is_some() {
        return;
    }

    let mut resolved = cache_dir.to_path_buf();
    resolved.push("media-cache");
    let _ = MEDIA_CACHE_DIR.set(resolved);
}

/// Remove all cached media files from the media cache directory. Called on app
/// startup so stale media from a previous session does not accumulate on disk.
pub fn clear_media_cache() {
    let cache_dir = media_cache_dir();
    if !cache_dir.exists() {
        return;
    }

    if let Err(error) = fs::remove_dir_all(&cache_dir) {
        warn!("Failed to clear media cache directory: {error}");
        return;
    }

    let _ = fs::create_dir_all(&cache_dir);
}

pub async fn cache_mxc_media_to_local_path(
    client: &matrix_sdk::Client,
    raw_url: &str,
) -> Option<String> {
    if !raw_url.starts_with("mxc://") {
        return None;
    }

    if let Some(cached_path) = cached_media_path_for_source_url(raw_url) {
        return Some(cached_path);
    }

    let mxc_uri = matrix_sdk::ruma::OwnedMxcUri::from(raw_url);
    let request = MediaRequestParameters {
        source: MediaSource::Plain(mxc_uri),
        format: MediaFormat::File,
    };

    let bytes = match client.media().get_media_content(&request, true).await {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!("Failed to fetch MXC image media content: {error}");
            return None;
        }
    };

    let file_stem = mxc_image_cache_key(raw_url, &bytes);
    let extension = image_extension_from_raw_url(raw_url);
    let mime_type = mime_type_from_extension(extension);

    let request = NormalizedImageLoad::builder()
        .bytes(&bytes)
        .file_stem(file_stem)
        .extension(extension)
        .mime_type(mime_type)
        .build()?;

    let resolved_path = persist_normalized_image(&request)?;
    register_cached_media_path(raw_url, &resolved_path);
    Some(resolved_path)
}

pub async fn resolve_pack_media_url(client: &matrix_sdk::Client, raw_url: &str) -> Option<String> {
    if raw_url.starts_with("mxc://") {
        return cache_mxc_media_to_local_path(client, raw_url).await;
    }

    if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
        if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
            return cache_mxc_media_to_local_path(client, &mxc_url).await;
        }

        warn!(
            "Ignoring non-Matrix HTTP media URL because image fetching is Matrix SDK-only: {}",
            raw_url
        );
        return None;
    }

    None
}

pub fn canonical_pack_source_url(raw_url: &str) -> String {
    if raw_url.starts_with("mxc://") {
        return raw_url.to_owned();
    }

    if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
        return mxc_url;
    }

    raw_url.to_owned()
}

pub fn cache_event_image(bytes: &[u8], key_parts: ImageCacheKeyParts) -> Option<String> {
    let extension = image_extension_from_mime(&key_parts.mime_type);
    let file_stem = image_cache_key(&key_parts);

    let request = NormalizedImageLoad::builder()
        .bytes(bytes)
        .file_stem(file_stem)
        .extension(extension)
        .mime_type(key_parts.mime_type)
        .build()?;

    persist_normalized_image(&request)
}

/// Cache arbitrary media bytes (e.g. video) to the media cache directory and
/// return the resolved `asset://` URL. Unlike `cache_event_image`, this does
/// not normalize or re-encode the bytes; it writes them verbatim so the file
/// keeps its original container/codec.
pub fn cache_media_bytes(bytes: &[u8], file_stem: &str, mime_type: &str) -> Option<String> {
    let extension = media_extension_from_mime(mime_type);

    let request = NormalizedImageLoad::builder()
        .bytes(bytes)
        .file_stem(file_stem)
        .extension(extension)
        .mime_type(mime_type)
        .build()?;

    persist_normalized_image(&request)
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

pub fn load_media_bytes_from_resolved_url(raw_url: &str) -> Option<Vec<u8>> {
    let file_path = resolved_media_file_path(raw_url)?;
    fs::read(file_path).ok()
}

/// Whether a resolved media URL (`asset://`, `file://`, or absolute path) still
/// points to an existing file on disk. Used to detect stale cached media URLs
/// so the caller can re-fetch from the server.
pub fn media_url_is_available(raw_url: &str) -> bool {
    resolved_media_file_path(raw_url).is_some_and(|path| path.exists())
}

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

fn resolved_media_file_path(raw_url: &str) -> Option<PathBuf> {
    if raw_url.starts_with("asset://") {
        let parsed = url::Url::parse(raw_url).ok()?;
        let path = percent_decode_str(parsed.path()).decode_utf8().ok()?;
        if path.is_empty() || path == "/" {
            return None;
        }

        return Some(PathBuf::from(path.as_ref()));
    }

    if raw_url.starts_with("file://") {
        let parsed = url::Url::parse(raw_url).ok()?;
        return parsed.to_file_path().ok();
    }

    if raw_url.starts_with('/') {
        return Some(PathBuf::from(raw_url));
    }

    None
}

fn persist_normalized_image(request: &NormalizedImageLoad) -> Option<String> {
    persist_cached_media_asset(request)
}

fn persist_cached_media_asset(request: &NormalizedImageLoad) -> Option<String> {
    let cache_dir = media_cache_dir();
    if let Err(error) = fs::create_dir_all(&cache_dir) {
        warn!("Failed to initialize media cache directory: {error}");
        return None;
    }

    let file_name = format!("{}.{}", request.file_stem, request.extension);
    let final_path = cache_dir.join(file_name);

    if final_path.exists() {
        return Some(to_asset_storage_url(&final_path));
    }

    let temp_path = cache_dir.join(format!("{}.tmp", request.file_stem));
    if let Err(error) = fs::write(&temp_path, &request.bytes) {
        warn!("Failed to write cached media file: {error}");
        return None;
    }

    if let Err(error) = fs::rename(&temp_path, &final_path) {
        let _ = fs::remove_file(&temp_path);
        if final_path.exists() {
            return Some(to_asset_storage_url(&final_path));
        }
        warn!("Failed to finalize cached media file: {error}");
        return None;
    }

    Some(to_asset_storage_url(&final_path))
}

fn to_asset_storage_url(path: &Path) -> String {
    let absolute = path.to_string_lossy();
    let encoded = percent_encode_asset_path(absolute.as_ref());
    format!("asset://localhost/{encoded}")
}

fn percent_encode_asset_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        let is_unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');

        if is_unreserved {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }

    encoded
}

fn mxc_image_cache_key(raw_url: &str, bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    raw_url.hash(&mut hasher);
    bytes.len().hash(&mut hasher);
    format!("img-{:016x}", hasher.finish())
}

fn image_cache_key(parts: &ImageCacheKeyParts) -> String {
    let mut hasher = DefaultHasher::new();
    parts.event_id.hash(&mut hasher);
    parts.origin_server_ts.hash(&mut hasher);
    parts.room_id.hash(&mut hasher);
    parts.source_key.hash(&mut hasher);
    parts.mime_type.hash(&mut hasher);
    parts.bytes_len.hash(&mut hasher);
    format!("img-{:016x}", hasher.finish())
}

fn image_extension_from_raw_url(raw_url: &str) -> &'static str {
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

fn mxc_from_matrix_media_download_url(raw_url: &str) -> Option<String> {
    let parsed = url::Url::parse(raw_url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }

    let segments: Vec<_> = parsed.path_segments()?.collect();
    let download_index = segments.windows(4).position(|window| {
        window.first() == Some(&"_matrix")
            && window.get(1) == Some(&"media")
            && window.get(3) == Some(&"download")
    })?;

    let server_name = segments.get(download_index + 4)?;
    let media_id = segments.get(download_index + 5)?;

    if server_name.is_empty() || media_id.is_empty() {
        return None;
    }

    Some(format!("mxc://{server_name}/{media_id}"))
}

fn mime_type_from_extension(extension: &str) -> &'static str {
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

fn cached_media_path_for_source_url(source_url: &str) -> Option<String> {
    let cached_path = {
        let cache = cached_media_urls();
        let lock = cache.lock().ok()?;
        lock.get(source_url).cloned()
    }?;

    if resolved_media_file_path(&cached_path).is_some_and(|path| path.exists()) {
        return Some(cached_path);
    }

    if let Ok(mut cache) = cached_media_urls().lock() {
        cache.remove(source_url);
    }

    None
}

fn register_cached_media_path(source_url: &str, resolved_path: &str) {
    if let Ok(mut cache) = cached_media_urls().lock() {
        cache.insert(source_url.to_owned(), resolved_path.to_owned());
    }
}

fn cached_media_urls() -> &'static Mutex<HashMap<String, String>> {
    CACHED_MEDIA_URLS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn media_cache_dir() -> PathBuf {
    MEDIA_CACHE_DIR
        .get_or_init(|| {
            let mut dir = std::env::temp_dir();
            dir.push("singularity");
            dir.push("media-cache");
            dir
        })
        .clone()
}

pub fn media_cache_dir_path() -> PathBuf {
    media_cache_dir()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        canonical_pack_source_url, image_extension_from_mime, load_media_bytes_from_resolved_url,
        media_extension_from_mime, media_url_is_available, percent_encode_asset_path,
        resolve_pack_media_url, to_asset_storage_url, ImageCacheKeyParts, NormalizedImageLoad,
        NormalizedImageLoadBuilder,
    };

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

    #[test]
    fn image_cache_key_builder_requires_fields() {
        let missing = ImageCacheKeyParts::builder().mime_type("image/png").build();
        assert!(missing.is_none());

        let complete = ImageCacheKeyParts::builder()
            .event_id(Some("$abc"))
            .origin_server_ts(Some(123))
            .room_id(Some("!room:server"))
            .source_key(Some("mxc://server/media"))
            .mime_type("image/png")
            .bytes_len(10)
            .build();
        assert!(complete.is_some());
    }

    #[test]
    fn normalized_image_load_builder_requires_fields() {
        let missing = NormalizedImageLoadBuilder::default().file_stem("x").build();
        assert!(missing.is_none());

        let complete = NormalizedImageLoad::builder()
            .bytes(&[1, 2, 3])
            .file_stem("img-1")
            .extension("png")
            .mime_type("image/png")
            .build();
        assert!(complete.is_some());
    }

    #[test]
    fn canonical_pack_source_url_converts_matrix_download_http_url() {
        let canonical = canonical_pack_source_url(
            "https://matrix.example.org/_matrix/media/v3/download/media.example.org/abc123",
        );
        assert_eq!(canonical, "mxc://media.example.org/abc123");
    }

    #[tokio::test]
    async fn resolve_pack_media_url_rejects_non_matrix_http_urls() {
        let homeserver_url = url::Url::parse("https://example.org")
            .expect("homeserver URL should parse for test setup");
        let client = matrix_sdk::Client::new(homeserver_url)
            .await
            .expect("client should construct for URL-only validation");

        let resolved = resolve_pack_media_url(&client, "https://example.org/image.png").await;
        assert!(resolved.is_none());
    }

    #[test]
    fn percent_encodes_absolute_asset_path() {
        let encoded = percent_encode_asset_path("/home/lux/.cache/media-cache/img-123.bin");
        assert_eq!(
            encoded,
            "%2Fhome%2Flux%2F.cache%2Fmedia-cache%2Fimg-123.bin"
        );
    }

    #[test]
    fn asset_url_round_trips_to_original_path() {
        let dir = std::env::temp_dir().join("singularity-test-assets");
        fs::create_dir_all(&dir).expect("create temp media dir");
        let file = dir.join("img-123.png");
        fs::write(&file, &[1, 2, 3]).expect("write temp media file");

        let url = to_asset_storage_url(&file);
        assert!(url.starts_with("asset://localhost/"));

        assert!(media_url_is_available(&url));
        assert_eq!(
            load_media_bytes_from_resolved_url(&url),
            Some(vec![1, 2, 3]),
        );

        fs::remove_file(&file).expect("clean up temp media file");
        fs::remove_dir(&dir).expect("clean up temp media dir");
    }
}
