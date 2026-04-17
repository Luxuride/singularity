use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use log::warn;
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::ruma::events::room::MediaSource;
use percent_encoding::percent_decode_str;
use tauri::Manager;
static MEDIA_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
static IN_MEMORY_MEDIA_CACHE: OnceLock<Mutex<InMemoryMediaCache>> = OnceLock::new();
static CACHED_MEDIA_URLS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static MEDIA_STORAGE_MODE: AtomicU8 = AtomicU8::new(MediaStorageMode::InMemory as u8);

const MAX_IN_MEMORY_MEDIA_ITEMS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MediaStorageMode {
    InMemory = 0,
    AssetStorage = 1,
}

impl MediaStorageMode {
    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::AssetStorage,
            _ => Self::InMemory,
        }
    }
}

#[derive(Clone)]
struct InMemoryMediaValue {
    bytes: Vec<u8>,
    mime_type: String,
}

#[derive(Default)]
struct InMemoryMediaCache {
    values: HashMap<String, InMemoryMediaValue>,
    order: VecDeque<String>,
}

impl InMemoryMediaCache {
    fn insert(&mut self, key: String, value: InMemoryMediaValue) {
        if self.values.contains_key(&key) {
            self.values.insert(key.clone(), value);
            self.touch(&key);
            return;
        }

        self.values.insert(key.clone(), value);
        self.order.push_back(key);

        while self.values.len() > MAX_IN_MEMORY_MEDIA_ITEMS {
            if let Some(evicted) = self.order.pop_front() {
                self.values.remove(&evicted);
            } else {
                break;
            }
        }
    }

    fn get(&mut self, key: &str) -> Option<InMemoryMediaValue> {
        let value = self.values.get(key)?.clone();
        self.touch(key);
        Some(value)
    }

    fn clear(&mut self) {
        self.values.clear();
        self.order.clear();
    }

    fn touch(&mut self, key: &str) {
        if let Some(position) = self.order.iter().position(|entry| entry == key) {
            self.order.remove(position);
        }
        self.order.push_back(key.to_owned());
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ImageCacheKeyParts {
    event_id: Option<String>,
    origin_server_ts: Option<u64>,
    room_id: Option<String>,
    source_key: Option<String>,
    mime_type: String,
    bytes_len: usize,
}

impl ImageCacheKeyParts {
    pub(crate) fn builder() -> ImageCacheKeyPartsBuilder {
        ImageCacheKeyPartsBuilder::default()
    }
}

#[derive(Default)]
pub(crate) struct ImageCacheKeyPartsBuilder {
    event_id: Option<String>,
    origin_server_ts: Option<u64>,
    room_id: Option<String>,
    source_key: Option<String>,
    mime_type: Option<String>,
    bytes_len: Option<usize>,
}

impl ImageCacheKeyPartsBuilder {
    pub(crate) fn event_id<T>(mut self, event_id: Option<T>) -> Self
    where
        T: Into<String>,
    {
        self.event_id = event_id.map(Into::into);
        self
    }

    pub(crate) fn origin_server_ts(mut self, origin_server_ts: Option<u64>) -> Self {
        self.origin_server_ts = origin_server_ts;
        self
    }

    pub(crate) fn room_id<T>(mut self, room_id: Option<T>) -> Self
    where
        T: Into<String>,
    {
        self.room_id = room_id.map(Into::into);
        self
    }

    pub(crate) fn source_key<T>(mut self, source_key: Option<T>) -> Self
    where
        T: Into<String>,
    {
        self.source_key = source_key.map(Into::into);
        self
    }

    pub(crate) fn mime_type<T>(mut self, mime_type: T) -> Self
    where
        T: Into<String>,
    {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub(crate) fn bytes_len(mut self, bytes_len: usize) -> Self {
        self.bytes_len = Some(bytes_len);
        self
    }

    pub(crate) fn build(self) -> Option<ImageCacheKeyParts> {
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
pub(crate) struct NormalizedImageLoad {
    bytes: Vec<u8>,
    file_stem: String,
    extension: String,
    mime_type: String,
}

impl NormalizedImageLoad {
    pub(crate) fn builder() -> NormalizedImageLoadBuilder {
        NormalizedImageLoadBuilder::default()
    }
}

#[derive(Default)]
pub(crate) struct NormalizedImageLoadBuilder {
    bytes: Option<Vec<u8>>,
    file_stem: Option<String>,
    extension: Option<String>,
    mime_type: Option<String>,
}

impl NormalizedImageLoadBuilder {
    pub(crate) fn bytes(mut self, bytes: &[u8]) -> Self {
        self.bytes = Some(bytes.to_vec());
        self
    }

    pub(crate) fn file_stem<T>(mut self, file_stem: T) -> Self
    where
        T: Into<String>,
    {
        self.file_stem = Some(file_stem.into());
        self
    }

    pub(crate) fn extension<T>(mut self, extension: T) -> Self
    where
        T: Into<String>,
    {
        self.extension = Some(extension.into());
        self
    }

    pub(crate) fn mime_type<T>(mut self, mime_type: T) -> Self
    where
        T: Into<String>,
    {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub(crate) fn build(self) -> Option<NormalizedImageLoad> {
        Some(NormalizedImageLoad {
            bytes: self.bytes?,
            file_stem: self.file_stem?,
            extension: self.extension?,
            mime_type: self.mime_type?,
        })
    }
}

pub(crate) fn media_storage_mode() -> MediaStorageMode {
    MediaStorageMode::from_u8(MEDIA_STORAGE_MODE.load(Ordering::Relaxed))
}

pub(crate) fn set_media_storage_mode(mode: MediaStorageMode) {
    MEDIA_STORAGE_MODE.store(mode as u8, Ordering::Relaxed);
    clear_cached_media_urls();

    if matches!(mode, MediaStorageMode::AssetStorage) {
        clear_in_memory_media_cache();
    }
}

pub(crate) fn initialize_media_cache_dir<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) {
    if MEDIA_CACHE_DIR.get().is_some() {
        return;
    }

    let mut cache_dir = match app_handle.path().app_cache_dir() {
        Ok(path) => path,
        Err(error) => {
            warn!("Failed to resolve app cache directory, falling back to temp dir: {error}");
            let mut fallback = std::env::temp_dir();
            fallback.push("singularity");
            fallback
        }
    };

    cache_dir.push("media-cache");
    let _ = MEDIA_CACHE_DIR.set(cache_dir);
}

pub(crate) fn handle_media_protocol_request(
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let media_key = request.uri().path().trim_start_matches('/');
    if media_key.is_empty() {
        return build_protocol_response(
            tauri::http::StatusCode::BAD_REQUEST,
            "text/plain; charset=utf-8",
            b"missing media key".to_vec(),
        );
    }

    let Some((bytes, mime_type)) = load_cached_media_from_memory(media_key) else {
        return build_protocol_response(
            tauri::http::StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            b"media not found".to_vec(),
        );
    };

    build_protocol_response(tauri::http::StatusCode::OK, &mime_type, bytes)
}

pub(crate) async fn cache_mxc_media_to_local_path(
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

pub(crate) async fn resolve_pack_media_url(
    client: &matrix_sdk::Client,
    raw_url: &str,
) -> Option<String> {
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

pub(crate) fn canonical_pack_source_url(raw_url: &str) -> String {
    if raw_url.starts_with("mxc://") {
        return raw_url.to_owned();
    }

    if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
        return mxc_url;
    }

    raw_url.to_owned()
}

pub(crate) fn cache_event_image(bytes: &[u8], key_parts: ImageCacheKeyParts) -> Option<String> {
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

pub(crate) fn load_media_bytes_from_resolved_url(raw_url: &str) -> Option<Vec<u8>> {
    if let Some(media_key) = matrix_media_key_from_url(raw_url) {
        return load_cached_media_from_memory(&media_key).map(|(bytes, _)| bytes);
    }

    let file_path = resolved_media_file_path(raw_url)?;
    fs::read(file_path).ok()
}

pub(crate) fn image_extension_from_mime(mime_type: &str) -> &'static str {
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

fn build_protocol_response(
    status: tauri::http::StatusCode,
    mime_type: &str,
    body: Vec<u8>,
) -> tauri::http::Response<Vec<u8>> {
    match tauri::http::Response::builder()
        .status(status)
        .header(tauri::http::header::CONTENT_TYPE, mime_type)
        .body(body)
    {
        Ok(response) => response,
        Err(_) => tauri::http::Response::new(Vec::new()),
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

fn matrix_media_key_from_url(raw_url: &str) -> Option<String> {
    if !raw_url.starts_with("matrix-media://") {
        return None;
    }

    let parsed = url::Url::parse(raw_url).ok()?;
    let media_key = parsed.path().trim_start_matches('/');
    if media_key.is_empty() {
        return None;
    }

    Some(media_key.to_owned())
}

fn load_cached_media_from_memory(media_key: &str) -> Option<(Vec<u8>, String)> {
    let cache = in_memory_media_cache();
    let mut lock = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return None,
    };

    let entry = lock.get(media_key)?;
    Some((entry.bytes, entry.mime_type))
}

fn persist_normalized_image(request: &NormalizedImageLoad) -> Option<String> {
    if matches!(media_storage_mode(), MediaStorageMode::InMemory) {
        return persist_cached_media_in_memory(request);
    }

    persist_cached_media_asset(request)
}

fn persist_cached_media_in_memory(request: &NormalizedImageLoad) -> Option<String> {
    let media_key = format!("{}.{}", request.file_stem, request.extension);

    let cache = in_memory_media_cache();
    let mut lock = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return None,
    };

    lock.insert(
        media_key.clone(),
        InMemoryMediaValue {
            bytes: request.bytes.clone(),
            mime_type: request.mime_type.clone(),
        },
    );

    Some(to_in_memory_media_url(&media_key))
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

fn clear_in_memory_media_cache() {
    if let Ok(mut cache) = in_memory_media_cache().lock() {
        cache.clear();
    }

    clear_cached_media_urls();
}

fn clear_cached_media_urls() {
    if let Ok(mut cached_urls) = cached_media_urls().lock() {
        cached_urls.clear();
    }
}

fn in_memory_media_cache() -> &'static Mutex<InMemoryMediaCache> {
    IN_MEMORY_MEDIA_CACHE.get_or_init(|| Mutex::new(InMemoryMediaCache::default()))
}

fn cached_media_urls() -> &'static Mutex<HashMap<String, String>> {
    CACHED_MEDIA_URLS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_media_path_for_source_url(source_url: &str) -> Option<String> {
    let cached_path = {
        let cache = cached_media_urls();
        let lock = cache.lock().ok()?;
        lock.get(source_url).cloned()
    }?;

    if let Some(media_key) = matrix_media_key_from_url(&cached_path) {
        if load_cached_media_from_memory(&media_key).is_some() {
            return Some(cached_path);
        }
    } else if resolved_media_file_path(&cached_path).is_some_and(|path| path.exists()) {
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

fn to_in_memory_media_url(media_key: &str) -> String {
    format!("matrix-media://localhost/{media_key}")
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

#[cfg(test)]
mod tests {
    use super::{
        canonical_pack_source_url, image_extension_from_mime, percent_encode_asset_path,
        resolve_pack_media_url, ImageCacheKeyParts, NormalizedImageLoad,
        NormalizedImageLoadBuilder,
    };

    #[test]
    fn image_extension_from_mime_is_stable() {
        assert_eq!(image_extension_from_mime("image/jpeg"), "jpg");
        assert_eq!(image_extension_from_mime("image/png"), "png");
        assert_eq!(image_extension_from_mime("image/unknown"), "bin");
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
}
