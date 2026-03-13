use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::storage;

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixChatSummary {
    pub room_id: String,
    pub display_name: String,
    pub encrypted: bool,
    pub joined_members: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetChatsResponse {
    pub chats: Vec<MatrixChatSummary>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedChatsCache {
    chats: Vec<MatrixChatSummary>,
}

#[tauri::command]
pub async fn matrix_get_chats(
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatsResponse, String> {
    auth_state.restore_client_from_disk_if_needed(&app_handle).await?;
    let client = auth_state.client()?;

    if let Err(error) = sync_client_rooms_once(&client).await {
        if let Some(cached_chats) = load_cached_chats(&app_handle)? {
            return Ok(MatrixGetChatsResponse { chats: cached_chats });
        }

        return Err(error);
    }

    let chats = collect_chat_summaries(&client).await;

    store_cached_chats(&app_handle, &chats)?;

    Ok(MatrixGetChatsResponse { chats })
}

pub(crate) async fn sync_client_rooms_once(client: &matrix_sdk::Client) -> Result<(), String> {
    client
        .sync_once(matrix_sdk::config::SyncSettings::default().timeout(Duration::from_secs(5)))
        .await
        .map_err(|error| format!("Failed to sync Matrix rooms: {error}"))?;

    Ok(())
}

pub(crate) async fn collect_chat_summaries(client: &matrix_sdk::Client) -> Vec<MatrixChatSummary> {
    let mut chats = Vec::new();

    for room in client.joined_rooms() {
        let display_name = room
            .display_name()
            .await
            .map(|name| name.to_string())
            .unwrap_or_else(|_| room.room_id().to_string());

        let encrypted = room
            .latest_encryption_state()
            .await
            .map(|state| state.is_encrypted())
            .unwrap_or(false);
        let joined_members = room.joined_members_count();

        chats.push(MatrixChatSummary {
            room_id: room.room_id().to_string(),
            display_name,
            encrypted,
            joined_members,
        });
    }

    chats.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
    chats
}

fn chats_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = storage::app_data_dir(app)?;
    Ok(data_dir.join("matrix-chats-cache.json"))
}

pub(crate) fn load_cached_chats(app: &AppHandle) -> Result<Option<Vec<MatrixChatSummary>>, String> {
    let path = chats_cache_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read chats cache: {error}"))?;

    let parsed = serde_json::from_str::<PersistedChatsCache>(&raw)
        .map_err(|error| format!("Failed to parse chats cache: {error}"))?;

    Ok(Some(parsed.chats))
}

pub(crate) fn store_cached_chats(app: &AppHandle, chats: &[MatrixChatSummary]) -> Result<(), String> {
    let path = chats_cache_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create app data directory: {error}"))?;
    }

    let raw = serde_json::to_string(&PersistedChatsCache { chats: chats.to_vec() })
        .map_err(|error| format!("Failed to serialize chats cache: {error}"))?;

    fs::write(path, raw).map_err(|error| format!("Failed to persist chats cache: {error}"))?;
    Ok(())
}
