//! Secure loopback HTTP server for serving cached video files.
//!
//! WebKitGTK's GStreamer media pipeline rejects custom URI schemes such as
//! `asset://`, so videos are served over loopback HTTP instead. This server:
//!
//! - binds exclusively to `127.0.0.1` (never a non-loopback interface),
//! - serves only files explicitly registered via an opaque capability token,
//! - never accepts filesystem paths from the request,
//! - supports the byte ranges WebKitGTK needs for playback and seeking,
//! - streams file chunks instead of buffering whole videos in memory.
//!
//! The capability token is the authorization: a high port alone is not
//! security. Tokens are cryptographically random and expire after a bounded
//! TTL, and the registry is capped so it cannot grow without limit.
//!
//! The HTTP serving itself is delegated to `axum`; this module only owns the
//! registry, the port-selection strategy, and the range/response policy.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rand::RngCore;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;

/// Preferred unprivileged port in the dynamic/private range.
const PREFERRED_PORT: u16 = 43110;
/// Number of random high-port fallback attempts before falling back to an
/// OS-assigned ephemeral port.
const MAX_FALLBACK_ATTEMPTS: u16 = 20;
/// Lower bound of the dynamic/private port range.
const HIGH_PORT_MIN: u16 = 49152;
/// Upper bound of the dynamic/private port range.
const HIGH_PORT_MAX: u16 = 65535;
/// Number of random bytes in a capability token (hex-encoded => 32 chars).
const CAPABILITY_BYTES: usize = 16;
/// How long a registered video capability remains valid.
const REGISTRY_TTL: Duration = Duration::from_secs(3600);
/// Maximum number of concurrently registered video capabilities.
const MAX_REGISTRY_ENTRIES: usize = 256;

#[derive(Clone)]
struct RegisteredVideo {
    path: PathBuf,
    mime_type: String,
    created: Instant,
}

/// Shared registry of registered videos, keyed by capability token.
#[derive(Clone, Default)]
struct Registry(Arc<Mutex<HashMap<String, RegisteredVideo>>>);

impl Registry {
    fn get(&self, capability: &str) -> Option<RegisteredVideo> {
        let mut registry = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        evict_expired(&mut registry);
        registry.get(capability).cloned()
    }

    fn insert(&self, capability: String, video: RegisteredVideo) {
        let mut registry = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        evict_expired(&mut registry);
        if registry.len() >= MAX_REGISTRY_ENTRIES {
            evict_oldest(&mut registry);
        }
        registry.insert(capability, video);
    }
}

/// A running loopback video server.
pub struct VideoServer {
    addr: SocketAddr,
    registry: Registry,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
    #[allow(dead_code)]
    handle: tokio::task::JoinHandle<()>,
}

impl VideoServer {
    /// Bind to `127.0.0.1` and start serving. Prefers a fixed high port, then
    /// tries a bounded set of random high ports, then lets the OS assign an
    /// ephemeral port. Must be called from within a Tokio runtime context.
    pub async fn start() -> Result<Self, String> {
        let listener = bind_with_fallback().await?;
        let addr = listener
            .local_addr()
            .map_err(|error| format!("Failed to read loopback listener address: {error}"))?;

        let registry = Registry::default();
        let app = Router::new()
            .route("/v/{capability}", get(serve_video))
            .with_state(registry.clone());

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await;
        });

        Ok(Self {
            addr,
            registry,
            shutdown_tx,
            handle,
        })
    }

    /// The bound loopback address (`127.0.0.1:<port>`).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Start the server preferring a specific port. Used by tests to exercise
    /// the fallback path deterministically.
    #[cfg(test)]
    async fn start_with_preferred(preferred: u16) -> Result<Self, String> {
        let listener = bind_with_preferred(preferred).await?;
        let addr = listener
            .local_addr()
            .map_err(|error| format!("Failed to read loopback listener address: {error}"))?;

        let registry = Registry::default();
        let app = Router::new()
            .route("/v/{capability}", get(serve_video))
            .with_state(registry.clone());

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.changed().await;
                })
                .await;
        });

        Ok(Self {
            addr,
            registry,
            shutdown_tx,
            handle,
        })
    }

    /// Register a cached video file and return an opaque, capability-scoped
    /// HTTP URL for it. Returns `None` if the file does not exist.
    pub fn register_video(&self, path: &Path, mime_type: &str) -> Option<String> {
        if !path.is_file() {
            return None;
        }

        let capability = random_capability();
        self.registry.insert(
            capability.clone(),
            RegisteredVideo {
                path: path.to_path_buf(),
                mime_type: mime_type.to_string(),
                created: Instant::now(),
            },
        );

        Some(format!("http://{}/v/{capability}", self.addr))
    }

    /// Stop the accept loop. Pending connections are dropped; the process may
    /// continue to run.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

/// Bind to `127.0.0.1` with the preferred-port / fallback / ephemeral strategy.
async fn bind_with_fallback() -> Result<TcpListener, String> {
    bind_with_preferred(PREFERRED_PORT).await
}

async fn bind_with_preferred(preferred: u16) -> Result<TcpListener, String> {
    if let Ok(listener) = TcpListener::bind(("127.0.0.1", preferred)).await {
        return Ok(listener);
    }

    for _ in 0..MAX_FALLBACK_ATTEMPTS {
        let port = random_high_port();
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return Ok(listener);
        }
    }

    TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|error| format!("Failed to bind loopback video server: {error}"))
}

fn random_high_port() -> u16 {
    let range = (HIGH_PORT_MAX - HIGH_PORT_MIN + 1) as u32;
    HIGH_PORT_MIN + (rand::thread_rng().next_u32() % range) as u16
}

fn random_capability() -> String {
    let mut bytes = [0u8; CAPABILITY_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn evict_expired(registry: &mut HashMap<String, RegisteredVideo>) {
    let now = Instant::now();
    registry.retain(|_, video| now.duration_since(video.created) < REGISTRY_TTL);
}

fn evict_oldest(registry: &mut HashMap<String, RegisteredVideo>) {
    if let Some(oldest) = registry
        .iter()
        .min_by_key(|(_, video)| video.created)
        .map(|(key, _)| key.clone())
    {
        registry.remove(&oldest);
    }
}

/// Validate a capability token from the URL path. Returns `None` for any
/// malformed token.
fn validate_capability(capability: &str) -> Option<String> {
    if capability.len() != CAPABILITY_BYTES * 2
        || !capability.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    Some(capability.to_string())
}

/// A parsed single byte range, resolved against the file length.
#[derive(Debug, PartialEq, Eq)]
struct ByteRange {
    start: u64,
    end: u64,
}

/// Parse a single `Range: bytes=...` header against `len`. Returns:
/// - `Some(None)` for no/empty/invalid range (serve the full resource),
/// - `Some(Some(range))` for a contained single range,
/// - `Err(())` for an unsatisfiable range (`416`).
fn parse_range(value: Option<&str>, len: u64) -> Result<Option<ByteRange>, ()> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(spec) = value.strip_prefix("bytes=") else {
        return Ok(None);
    };
    let spec = spec.trim();
    if spec.contains(',') {
        return Err(());
    }
    let Some((start, end)) = spec.split_once('-') else {
        return Ok(None);
    };
    let start = start.trim();
    let end = end.trim();

    // Suffix range: bytes=-N
    if start.is_empty() {
        if len == 0 {
            return Err(());
        }
        let Ok(suffix) = end.parse::<u64>() else {
            return Ok(None);
        };
        if suffix == 0 {
            return Err(());
        }
        let start = len.saturating_sub(suffix);
        return Ok(Some(ByteRange { start, end: len - 1 }));
    }

    let Ok(start) = start.parse::<u64>() else {
        return Ok(None);
    };
    if start >= len {
        return Err(());
    }
    let end = if end.is_empty() {
        len - 1
    } else {
        let Ok(end) = end.parse::<u64>() else {
            return Ok(None);
        };
        end.min(len - 1)
    };
    if end < start {
        return Err(());
    }
    Ok(Some(ByteRange { start, end }))
}

async fn serve_video(
    State(registry): State<Registry>,
    AxumPath(capability): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let Some(capability) = validate_capability(&capability) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(registered) = registry.get(&capability) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let mut file = match tokio::fs::File::open(&registered.path).await {
        Ok(file) => file,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let len = match file.metadata().await {
        Ok(meta) => meta.len(),
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let range_header = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let range = match parse_range(range_header, len) {
        Ok(range) => range,
        Err(()) => {
            let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{len}")).unwrap(),
            );
            return response;
        }
    };

    let Some(range) = range else {
        // Serve the full resource, streaming from disk.
        let body = Body::from_stream(ReaderStream::new(file));
        let mut response = Response::new(body);
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&registered.mime_type).unwrap_or(HeaderValue::from_static(
                "application/octet-stream",
            )),
        );
        response
            .headers_mut()
            .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from_str(&len.to_string()).unwrap());
        return response;
    };

    if file
        .seek(std::io::SeekFrom::Start(range.start))
        .await
        .is_err()
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let chunk_len = range.end - range.start + 1;
    let body = Body::from_stream(ReaderStream::new(file.take(chunk_len)));

    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::PARTIAL_CONTENT;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&registered.mime_type).unwrap_or(HeaderValue::from_static(
            "application/octet-stream",
        )),
    );
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, HeaderValue::from_str(&chunk_len.to_string()).unwrap());
    response.headers_mut().insert(
        header::CONTENT_RANGE,
        HeaderValue::from_str(&format!("bytes {}-{}/{}", range.start, range.end, len)).unwrap(),
    );

    response
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::SocketAddr;

    use super::{ByteRange, MAX_REGISTRY_ENTRIES, VideoServer, parse_range, validate_capability};

    async fn start_server() -> VideoServer {
        VideoServer::start().await.expect("start video server")
    }

    async fn http_request(
        addr: SocketAddr,
        method: &str,
        path: &str,
        range: Option<&str>,
    ) -> String {
        let client = reqwest::Client::new();
        let url = format!("http://{addr}{path}");
        let mut request = client.request(
            reqwest::Method::from_bytes(method.as_bytes()).expect("valid method"),
            &url,
        );
        if let Some(range) = range {
            request = request.header("Range", range);
        }
        let response = request.send().await.expect("send request");
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let content_range = response
            .headers()
            .get("content-range")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let body = response.text().await.expect("read body");
        format!(
            "STATUS {status}\nCONTENT-TYPE {content_type}\nCONTENT-RANGE {content_range}\nCONTENT-LENGTH {content_length}\nBODY {body}"
        )
    }

    fn status_line(response: &str) -> &str {
        response.lines().next().unwrap_or_default()
    }

    fn header(response: &str, name: &str) -> Option<String> {
        response.lines().find_map(|line| {
            let (key, value) = line.split_once(' ')?;
            if key.eq_ignore_ascii_case(name) {
                Some(value.to_string())
            } else {
                None
            }
        })
    }

    fn body(response: &str) -> &str {
        response
            .lines()
            .find_map(|line| line.strip_prefix("BODY "))
            .unwrap_or_default()
    }

    fn write_temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("singularity-video-server-test");
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join(name);
        fs::write(&path, bytes).expect("write temp file");
        path
    }

    #[test]
    fn validate_capability_accepts_valid_token() {
        let token = "0123456789abcdef0123456789abcdef";
        assert_eq!(validate_capability(token), Some(token.to_string()));
    }

    #[test]
    fn validate_capability_rejects_invalid_shapes() {
        assert_eq!(validate_capability(""), None);
        assert_eq!(validate_capability("short"), None);
        assert_eq!(validate_capability("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"), None);
    }

    #[test]
    fn parse_range_handles_full_and_bytes() {
        assert_eq!(parse_range(None, 100).unwrap(), None);
        assert_eq!(
            parse_range(Some("bytes=0-3"), 100).unwrap(),
            Some(ByteRange { start: 0, end: 3 })
        );
        assert_eq!(
            parse_range(Some("bytes=10-"), 100).unwrap(),
            Some(ByteRange { start: 10, end: 99 })
        );
        assert_eq!(
            parse_range(Some("bytes=-4"), 100).unwrap(),
            Some(ByteRange { start: 96, end: 99 })
        );
        assert_eq!(
            parse_range(Some("bytes=0-999"), 100).unwrap(),
            Some(ByteRange { start: 0, end: 99 })
        );
    }

    #[test]
    fn parse_range_rejects_invalid() {
        assert_eq!(parse_range(Some("items=0-3"), 100).unwrap(), None);
        assert_eq!(parse_range(Some("bytes=abc"), 100).unwrap(), None);
        assert!(parse_range(Some("bytes=100-"), 100).is_err());
        assert!(parse_range(Some("bytes=5-2"), 100).is_err());
        assert!(parse_range(Some("bytes=-0"), 100).is_err());
        assert!(parse_range(Some("bytes=0-3,5-8"), 100).is_err());
    }

    #[tokio::test]
    async fn serves_full_video_with_mime() {
        let server = start_server().await;
        let path = write_temp_file("full.mp4", b"0123456789");
        let url = server
            .register_video(&path, "video/mp4")
            .expect("register video");
        let capability = url.rsplit('/').next().unwrap();
        let response = http_request(server.addr(), "GET", &format!("/v/{capability}"), None).await;

        assert!(status_line(&response).contains("200"));
        assert_eq!(header(&response, "CONTENT-TYPE").as_deref(), Some("video/mp4"));
        assert_eq!(header(&response, "CONTENT-LENGTH").as_deref(), Some("10"));
        assert_eq!(body(&response), "0123456789");

        fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn serves_byte_range() {
        let server = start_server().await;
        let path = write_temp_file("range.mp4", b"0123456789");
        let url = server.register_video(&path, "video/mp4").unwrap();
        let capability = url.rsplit('/').next().unwrap();
        let response = http_request(
            server.addr(),
            "GET",
            &format!("/v/{capability}"),
            Some("bytes=2-5"),
        )
        .await;

        assert!(status_line(&response).contains("206"));
        assert_eq!(header(&response, "CONTENT-RANGE").as_deref(), Some("bytes 2-5/10"));
        assert_eq!(header(&response, "CONTENT-LENGTH").as_deref(), Some("4"));
        assert_eq!(body(&response), "2345");

        fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn serves_suffix_range() {
        let server = start_server().await;
        let path = write_temp_file("suffix.mp4", b"0123456789");
        let url = server.register_video(&path, "video/mp4").unwrap();
        let capability = url.rsplit('/').next().unwrap();
        let response = http_request(
            server.addr(),
            "GET",
            &format!("/v/{capability}"),
            Some("bytes=-4"),
        )
        .await;

        assert!(status_line(&response).contains("206"));
        assert_eq!(header(&response, "CONTENT-RANGE").as_deref(), Some("bytes 6-9/10"));
        assert_eq!(body(&response), "6789");

        fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn head_returns_headers_without_body() {
        let server = start_server().await;
        let path = write_temp_file("head.mp4", b"0123456789");
        let url = server.register_video(&path, "video/mp4").unwrap();
        let capability = url.rsplit('/').next().unwrap();
        let response = http_request(server.addr(), "HEAD", &format!("/v/{capability}"), None).await;

        assert!(status_line(&response).contains("200"));
        assert_eq!(header(&response, "CONTENT-LENGTH").as_deref(), Some("10"));
        assert_eq!(body(&response), "");

        fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn rejects_unknown_capability() {
        let server = start_server().await;
        let response = http_request(
            server.addr(),
            "GET",
            "/v/ffffffffffffffffffffffffffffffff",
            None,
        )
        .await;
        assert!(status_line(&response).contains("404"));
    }

    #[tokio::test]
    async fn rejects_non_video_path() {
        let server = start_server().await;
        let response = http_request(server.addr(), "GET", "/etc/passwd", None).await;
        assert!(status_line(&response).contains("404"));
    }

    #[tokio::test]
    async fn rejects_unsupported_method() {
        let server = start_server().await;
        let path = write_temp_file("method.mp4", b"0123456789");
        let url = server.register_video(&path, "video/mp4").unwrap();
        let capability = url.rsplit('/').next().unwrap();
        let response = http_request(server.addr(), "POST", &format!("/v/{capability}"), None).await;
        assert!(status_line(&response).contains("405"));

        fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn rejects_unsatisfiable_range() {
        let server = start_server().await;
        let path = write_temp_file("unsat.mp4", b"0123456789");
        let url = server.register_video(&path, "video/mp4").unwrap();
        let capability = url.rsplit('/').next().unwrap();
        let response = http_request(
            server.addr(),
            "GET",
            &format!("/v/{capability}"),
            Some("bytes=999-"),
        )
        .await;
        assert!(status_line(&response).contains("416"));

        fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn register_rejects_missing_file() {
        let server = start_server().await;
        let missing = std::env::temp_dir()
            .join("singularity-video-server-test")
            .join("missing.mp4");
        assert!(server.register_video(&missing, "video/mp4").is_none());
    }

    #[tokio::test]
    async fn falls_back_when_preferred_port_is_taken() {
        // Occupy a dedicated port so this test is independent of the fixed
        // preferred port and of other parallel tests.
        let blocker = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind blocker");
        let occupied = blocker.local_addr().unwrap().port();

        let server = VideoServer::start_with_preferred(occupied)
            .await
            .expect("start server with occupied preferred port");
        assert_ne!(server.addr().port(), occupied);
        drop(blocker);
    }

    #[tokio::test]
    async fn evicts_oldest_when_registry_is_full() {
        let server = start_server().await;
        let path = write_temp_file("evict.mp4", b"0123456789");
        let first = server.register_video(&path, "video/mp4").unwrap();
        let first_cap = first.rsplit('/').next().unwrap().to_string();

        for _ in 1..MAX_REGISTRY_ENTRIES {
            server.register_video(&path, "video/mp4").unwrap();
        }

        // The registry is now full; registering one more evicts the oldest.
        server.register_video(&path, "video/mp4").unwrap();
        let response = http_request(server.addr(), "GET", &format!("/v/{first_cap}"), None).await;
        assert!(status_line(&response).contains("404"));

        fs::remove_file(&path).ok();
    }
}