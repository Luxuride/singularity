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
