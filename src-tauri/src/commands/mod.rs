//! Tauri command adapters, split into logical modules. Each `#[tauri::command]`
//! is a thin wrapper that resolves managed state and delegates to the Tauri-free
//! library crates. No domain logic lives here.
//!
//! Layout (one module per domain, mirroring the lib crates):
//! - `auth`         — sign-in, session, recovery, logout
//! - `rooms`        — chat list, navigation, room image, join, update triggers
//! - `chat`         — messages, media, reactions, emoji, avatars, clipboard
//! - `settings`     — media storage settings
//! - `verification` — device verification / SAS flows
//!
//! Shared binder infrastructure (paths, event sink, media protocol) lives here.

pub mod auth;
pub mod chat;
pub mod rooms;
pub mod settings;
pub mod verification;

use tauri::{AppHandle, Emitter, Manager};
use types::Paths;

/// Resolve the application data + cache directories into a Tauri-free `Paths`.
pub fn resolve_paths(app: &AppHandle) -> Result<Paths, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))?;
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("Failed to resolve app cache directory: {error}"))?;
    Ok(Paths::new(data_dir, cache_dir))
}

/// `EventSink` backed by `AppHandle::emit`. The payload is already a
/// `serde_json::Value` from the domain crates, so it serializes directly.
pub struct AppHandleEventSink(AppHandle);

impl AppHandleEventSink {
    pub fn new(handle: AppHandle) -> Self {
        Self(handle)
    }
}

impl types::EventSink for AppHandleEventSink {
    fn emit(&self, event: &str, payload: &serde_json::Value) -> Result<(), String> {
        self.0
            .emit(event, payload)
            .map_err(|error| format!("Failed to emit event {event}: {error}"))
    }
}

/// Wrap the Tauri-free `assets::handle_media_request` into a `matrix-media://`
/// protocol response.
pub fn handle_media_protocol_request(
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

    let (bytes, mime_type) = assets::handle_media_request(media_key);
    let (Some(bytes), Some(mime_type)) = (bytes, mime_type) else {
        return build_protocol_response(
            tauri::http::StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            b"media not found".to_vec(),
        );
    };

    build_protocol_response(tauri::http::StatusCode::OK, &mime_type, bytes)
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
