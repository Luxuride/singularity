use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::protocol::storage_keys;
use crate::protocol::sync::sync_once_serialized;
use crate::storage;

use super::types::MatrixChatSummary;
use super::workers::collect_chat_summaries;
use super::RoomSnapshot;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedChatsCache {
    #[serde(default = "cache_schema_version")]
    version: u32,
    chats: Vec<MatrixChatSummary>,
}

const fn cache_schema_version() -> u32 {
    storage_keys::CACHE_SCHEMA_VERSION
}

fn chats_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage::app_data_file(app, storage_keys::CHATS_CACHE_FILE)
}

fn legacy_chats_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage::app_data_file(app, storage_keys::CHATS_CACHE_FILE_LEGACY)
}

pub(crate) fn load_cached_chats(app: &AppHandle) -> Result<Option<Vec<MatrixChatSummary>>, String> {
    let path = chats_cache_path(app)?;
    let path = if path.exists() {
        path
    } else {
        let legacy_path = legacy_chats_cache_path(app)?;
        if legacy_path.exists() {
            legacy_path
        } else {
            return Ok(None);
        }
    };

    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read chats cache: {error}"))?;

    let parsed = serde_json::from_str::<PersistedChatsCache>(&raw)
        .map_err(|error| format!("Failed to parse chats cache: {error}"))?;

    Ok(Some(parsed.chats))
}

pub(crate) fn store_cached_chats(
    app: &AppHandle,
    chats: &[MatrixChatSummary],
) -> Result<(), String> {
    let path = chats_cache_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create app data directory: {error}"))?;
    }

    let raw = serde_json::to_string(&PersistedChatsCache {
        version: cache_schema_version(),
        chats: chats.to_vec(),
    })
    .map_err(|error| format!("Failed to serialize chats cache: {error}"))?;

    fs::write(path, raw).map_err(|error| format!("Failed to persist chats cache: {error}"))?;
    Ok(())
}

pub(crate) async fn refresh_room_snapshot(
    app: &tauri::AppHandle,
    client: &matrix_sdk::Client,
    sync_timeout: std::time::Duration,
) -> Result<RoomSnapshot, String> {
    sync_once_serialized(
        client,
        matrix_sdk::config::SyncSettings::default().timeout(sync_timeout),
    )
    .await
    .map_err(|error| format!("Failed to sync Matrix rooms: {error}"))?;

    let chats = collect_chat_summaries(client).await;
    let _ = store_cached_chats(app, &chats);

    let mut current_snapshot = RoomSnapshot::new();
    for chat in chats {
        current_snapshot.insert(chat.room_id.clone(), chat);
    }

    Ok(current_snapshot)
}
