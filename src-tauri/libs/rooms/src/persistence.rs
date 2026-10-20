use std::collections::HashMap;
use std::sync::Arc;

use chat::cache_mxc_media_to_local_path;
use storage::AppDb;
use types::rooms::MatrixChatSummary;

use crate::workers::{collect_chat_summaries, RoomSnapshot};

pub async fn load_cached_chats(
    app_db: &Arc<AppDb>,
) -> Result<Option<Vec<MatrixChatSummary>>, String> {
    app_db.load_cached_chats().await
}

pub async fn load_cached_chat_image_sources(
    app_db: &Arc<AppDb>,
) -> Result<HashMap<String, String>, String> {
    app_db.load_cached_chat_image_sources().await
}

pub async fn store_cached_chats(
    app_db: &Arc<AppDb>,
    chats: &[MatrixChatSummary],
) -> Result<(), String> {
    app_db.store_chats(chats).await
}

pub async fn collect_and_store_chats(
    app_db: &Arc<AppDb>,
    client: &matrix_sdk::Client,
) -> Vec<MatrixChatSummary> {
    let cached_chats_by_room = load_cached_chats(app_db)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
        .into_iter()
        .map(|chat| (chat.room_id.clone(), chat))
        .collect::<HashMap<_, _>>();

    let cached_image_sources_by_room = load_cached_chat_image_sources(app_db)
        .await
        .unwrap_or_default();

    let mut chats = collect_chat_summaries(client).await;
    for chat in &mut chats {
        if let Some(cached) = cached_chats_by_room.get(chat.room_id.as_str()) {
            if chat.display_name == chat.room_id && !cached.display_name.is_empty() {
                chat.display_name = cached.display_name.clone();
            }
        }

        if let Some(source_url) = cached_image_sources_by_room.get(chat.room_id.as_str()) {
            chat.image_url = cache_mxc_media_to_local_path(client, source_url)
                .await
                .or_else(|| {
                    cached_chats_by_room
                        .get(chat.room_id.as_str())
                        .and_then(|c| c.image_url.clone())
                });
            continue;
        }

        if let Some(image_url) = cached_chats_by_room
            .get(chat.room_id.as_str())
            .and_then(|c| c.image_url.clone())
        {
            chat.image_url = Some(image_url);
        }
    }

    let _ = store_cached_chats(app_db, &chats).await;
    chats
}

pub async fn refresh_room_snapshot(
    app_db: &Arc<AppDb>,
    client: &matrix_sdk::Client,
    sync_timeout: std::time::Duration,
) -> Result<RoomSnapshot, String> {
    protocol::sync::sync_once_serialized(
        client,
        matrix_sdk::config::SyncSettings::default().timeout(sync_timeout),
    )
    .await
    .map_err(|error| format!("Failed to sync Matrix rooms: {error}"))?;

    let chats = collect_and_store_chats(app_db, client).await;

    let mut current_snapshot = RoomSnapshot::new();
    for chat in chats {
        current_snapshot.insert(chat.room_id.clone(), chat);
    }

    Ok(current_snapshot)
}
