use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::db::AppDb;
use crate::protocol::sync::sync_once_serialized;

use super::types::MatrixChatSummary;
use super::workers::collect_chat_summaries;
use super::RoomSnapshot;

pub(crate) fn load_cached_chats(app: &AppHandle) -> Result<Option<Vec<MatrixChatSummary>>, String> {
    let app_db = app.state::<AppDb>();
    app_db.load_cached_chats()
}

pub(crate) fn store_cached_chats(
    app: &AppHandle,
    chats: &[MatrixChatSummary],
) -> Result<(), String> {
    let app_db = app.state::<AppDb>();
    app_db.store_chats(chats)
}

pub(crate) async fn collect_and_store_chats(
    app: &tauri::AppHandle,
    client: &matrix_sdk::Client,
) -> Vec<MatrixChatSummary> {
    let cached_images_by_room = load_cached_chats(app)
        .ok()
        .flatten()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|chat| chat.image_url.map(|image_url| (chat.room_id, image_url)))
        .collect::<HashMap<_, _>>();

    let mut chats = collect_chat_summaries(client).await;
    for chat in &mut chats {
        if chat.image_url.is_none() {
            if let Some(image_url) = cached_images_by_room.get(chat.room_id.as_str()) {
                chat.image_url = Some(image_url.clone());
            }
        }
    }

    let _ = store_cached_chats(app, &chats);
    chats
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

    let chats = collect_and_store_chats(app, client).await;

    let mut current_snapshot = RoomSnapshot::new();
    for chat in chats {
        current_snapshot.insert(chat.room_id.clone(), chat);
    }

    Ok(current_snapshot)
}
