//! Profile-avatar change propagation.
//!
//! Matrix delivers profile changes as `m.room.member` state events in every
//! room the changed user shares, and a removed avatar clears `avatar_url` in
//! the event content. The room worker's snapshot diff only covers room images
//! resolved from `m.room.avatar` / the DM partner's profile at
//! `get_room_image` time, so an avatar change never reaches the already
//! rendered UI. These handlers close that gap: recache the member's avatar
//! media, refresh the image of every direct room mapped to them, and emit
//! `ROOM_UPDATED` / `PROFILE_AVATAR_UPDATED` so the frontend drops its stale
//! cached avatars and re-renders.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use matrix_sdk::room::Room;
use matrix_sdk::ruma::events::room::avatar::SyncRoomAvatarEvent;
use matrix_sdk::ruma::events::room::member::SyncRoomMemberEvent;
use matrix_sdk::Client;

use chat::cache_mxc_media_to_local_path;
use protocol::parse_user_id;
use storage::AppDb;
use types::event_paths;
use types::rooms::MatrixProfileAvatarUpdatedEvent;
use types::EventSink;

use crate::direct::direct_room_mapping;
use crate::persistence::{load_cached_chat_image_sources, load_cached_chats, store_cached_chats};

const SEEN_KEY_SEPARATOR: char = '\u{1F}';

/// Shared state the handlers need.
#[derive(Clone)]
pub struct AvatarChangeDeps {
    pub app_db: Arc<AppDb>,
    pub event_sink: Arc<dyn EventSink>,
}

/// Register the handlers for a signed-in client. Runs for the session's
/// lifetime; nothing needs to unsubscribe them.
pub fn watch_avatar_changes(client: &Client, deps: AvatarChangeDeps) {
    let member_handle = client.add_event_handler({
        let deps = deps.clone();
        move |event: SyncRoomMemberEvent, room: Room| {
            let deps = deps.clone();
            async move {
                let avatar_mxc = event
                    .as_original()
                    .and_then(|original| original.content.avatar_url.as_ref())
                    .map(ToString::to_string);
                handle_member_event(
                    &deps,
                    &room,
                    event.state_key().as_str().to_owned(),
                    avatar_mxc,
                )
                .await;
            }
        }
    });

    let room_avatar_handle = client.add_event_handler({
        move |event: SyncRoomAvatarEvent, room: Room| {
            let deps = deps.clone();
            async move {
                let avatar_mxc = event
                    .as_original()
                    .and_then(|original| original.content.url.as_ref())
                    .map(ToString::to_string);
                refresh_room_image(&deps, &room, avatar_mxc).await;
            }
        }
    });

    // Session-lifetime by design: dropping the handles would unsubscribe the
    // handlers, and there is no per-client store to park them in. They go
    // quiet with the client itself.
    std::mem::forget(member_handle);
    std::mem::forget(room_avatar_handle);
}

/// The last avatar mxc observed per (room, user). The first sighting of a
/// member event is recorded as a baseline without acting on it, so the initial
/// sync's replay of every room's membership does not eagerly re-fetch every
/// shared member's avatar; later changes against that baseline are applied
/// live. (Avatar changes made while the app was closed are picked up lazily:
/// rendering calls `get_room_image` / `get_user_avatar`, which recompute.)
fn seen_avatars() -> &'static Mutex<HashMap<String, Option<String>>> {
    static SEEN: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn handle_member_event(
    deps: &AvatarChangeDeps,
    room: &Room,
    user_id: String,
    avatar_mxc: Option<String>,
) {
    let key = format!("{}{}{}", room.room_id(), SEEN_KEY_SEPARATOR, user_id);
    let changed = {
        let Ok(mut seen) = seen_avatars().lock() else {
            return;
        };
        match seen.get(&key) {
            Some(previous) if *previous == avatar_mxc => false,
            _ => {
                let first_sighting = seen.insert(key, avatar_mxc.clone()).is_none();
                !first_sighting
            }
        }
    };

    if changed {
        // Record the event's avatar source before any re-render can read the
        // (one-sync-late) member profile, so get_user_avatar resolves the
        // current change, not the previous one.
        if let Err(error) = deps
            .app_db
            .set_user_avatar_source(&user_id, avatar_mxc.as_deref())
            .await
        {
            log::warn!("Failed to record avatar source for {user_id}: {error}");
        }

        apply_user_avatar_change(deps, &room.client(), &user_id, avatar_mxc).await;
    }
}

/// Apply a live avatar change for `user_id`: recache the avatar media, refresh
/// the images of their direct rooms, and tell the frontend whose avatars went
/// stale.
async fn apply_user_avatar_change(
    deps: &AvatarChangeDeps,
    client: &Client,
    user_id: &str,
    event_avatar_mxc: Option<String>,
) {
    let Ok(parsed_user_id) = parse_user_id(user_id) else {
        return;
    };

    // Recache the new avatar so per-message avatars resolve the fresh bytes
    // instead of the file the previous upload was persisted to.
    if let Some(mxc) = event_avatar_mxc.as_deref() {
        if cache_mxc_media_to_local_path(client, mxc).await.is_none() {
            log::warn!("Failed to cache avatar media for {mxc}");
        }
    }

    for (mapped_user_id, room_ids) in direct_room_mapping(client).await {
        if mapped_user_id != user_id {
            continue;
        }

        for room_id in room_ids {
            let Ok(parsed_room_id) = protocol::parse_room_id(&room_id) else {
                continue;
            };
            let Some(room) = client.get_room(&parsed_room_id) else {
                continue;
            };

            let member_avatar = room
                .get_member(&parsed_user_id)
                .await
                .ok()
                .flatten()
                .and_then(|member| member.avatar_url().map(ToString::to_string));
            refresh_room_image(deps, &room, member_avatar).await;
        }
    }

    let event = MatrixProfileAvatarUpdatedEvent {
        user_id: user_id.to_owned(),
    };
    if let Ok(payload) = serde_json::to_value(&event) {
        let _ = deps
            .event_sink
            .emit(event_paths::PROFILE_AVATAR_UPDATED, &payload);
    }
}

/// Recompute one room's image from `source_mxc`, persist the source, and emit
/// `ROOM_UPDATED` with the refreshed summary when the rendered image changed.
/// A no-op when the stored source already matches, so replayed events from a
/// sync do no work. Also used by `settings` to apply own-profile changes
/// immediately, without waiting for the sync echo.
pub async fn refresh_room_image(deps: &AvatarChangeDeps, room: &Room, source_mxc: Option<String>) {
    let room_id = room.room_id().to_string();

    let stored_source = load_cached_chat_image_sources(&deps.app_db)
        .await
        .ok()
        .and_then(|sources| sources.get(&room_id).cloned());
    if stored_source == source_mxc {
        return;
    }

    if let Err(error) = deps
        .app_db
        .set_chat_image_source(&room_id, source_mxc.as_deref())
        .await
    {
        log::warn!("Failed to store chat image source for {room_id}: {error}");
        return;
    }

    let image_url = match source_mxc.as_deref() {
        Some(mxc) => cache_mxc_media_to_local_path(&room.client(), mxc).await,
        None => None,
    };

    let Ok(Some(mut chats)) = load_cached_chats(&deps.app_db).await else {
        return;
    };
    let Some(chat) = chats.iter_mut().find(|chat| chat.room_id == room_id) else {
        return;
    };
    if chat.image_url == image_url {
        return;
    }
    chat.image_url = image_url;
    let updated = chat.clone();

    if store_cached_chats(&deps.app_db, &chats).await.is_ok() {
        if let Ok(payload) = serde_json::to_value(&updated) {
            let _ = deps.event_sink.emit(event_paths::ROOM_UPDATED, &payload);
        }
    }
}
