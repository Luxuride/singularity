use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use log::{debug, warn};
use matrix_sdk::deserialized_responses::{TimelineEvent, VerificationState};
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::events::{GlobalAccountDataEventType, StateEventType};
use matrix_sdk::ruma::events::reaction::ReactionEventContent;
use matrix_sdk::ruma::events::relation::Annotation;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::events::room::MediaSource;
use matrix_sdk::ruma::uint;
use serde_json::Value;
use url::Url;

use crate::protocol::events_schema::{parse_reaction_event, parse_timeline_message};

use super::types::{
    MatrixChatMessage, MatrixCustomEmoji, MatrixGetChatMessagesResponse, MatrixPickerCustomEmoji,
    MatrixMessageDecryptionStatus, MatrixMessageVerificationStatus, MatrixReactionSummary,
};

static MEDIA_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
static IN_MEMORY_MEDIA_CACHE: OnceLock<Mutex<InMemoryMediaCache>> = OnceLock::new();
static MEDIA_STORAGE_MODE: AtomicU8 = AtomicU8::new(MediaStorageMode::InMemory as u8);

const MAX_IN_MEMORY_MEDIA_ITEMS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MediaStorageMode {
    InMemory = 0,
    AssetStorage = 1,
}

impl MediaStorageMode {
    fn from_u8(value: u8) -> Self {
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

pub(crate) fn media_storage_mode() -> MediaStorageMode {
    MediaStorageMode::from_u8(MEDIA_STORAGE_MODE.load(Ordering::Relaxed))
}

pub(crate) fn set_media_storage_mode(mode: MediaStorageMode) {
    MEDIA_STORAGE_MODE.store(mode as u8, Ordering::Relaxed);
    if matches!(mode, MediaStorageMode::AssetStorage) {
        clear_in_memory_media_cache();
    }
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

fn load_cached_media_from_memory(media_key: &str) -> Option<(Vec<u8>, String)> {
    let cache = in_memory_media_cache();
    let mut lock = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return None,
    };

    let entry = lock.get(media_key)?;
    Some((entry.bytes, entry.mime_type))
}

fn clear_in_memory_media_cache() {
    if let Ok(mut cache) = in_memory_media_cache().lock() {
        cache.clear();
    }
}

fn in_memory_media_cache() -> &'static Mutex<InMemoryMediaCache> {
    IN_MEMORY_MEDIA_CACHE.get_or_init(|| Mutex::new(InMemoryMediaCache::default()))
}

fn to_in_memory_media_url(media_key: &str) -> String {
    format!("matrix-media://localhost/{media_key}")
}

pub(crate) async fn fetch_room_messages_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    from: Option<String>,
    limit: Option<u32>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
        .map_err(|_| String::from("roomId is invalid"))?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let response = room
        .messages(build_messages_options(from.clone(), limit))
        .await
        .map_err(|error| format!("Failed to read room messages: {error}"))?;

    let (mut messages, mut had_utd) = parse_message_chunk(client, response.chunk).await;
    let mut next_from = response.end;

    if had_utd && client.encryption().backups().are_enabled().await {
        if let Err(error) = client
            .encryption()
            .backups()
            .download_room_keys_for_room(&room_id)
            .await
        {
            warn!(
                "Failed to download backup keys for room {}: {}",
                room_id, error
            );
        } else if let Ok(retry_response) = room.messages(build_messages_options(from, limit)).await
        {
            let (retry_messages, retry_had_utd) =
                parse_message_chunk(client, retry_response.chunk).await;
            messages = retry_messages;
            had_utd = retry_had_utd;
            next_from = retry_response.end;
        }
    }

    debug!(
        "Fetched {} chat messages (utd_present={})",
        messages.len(),
        had_utd
    );

    Ok(MatrixGetChatMessagesResponse {
        room_id: room_id.to_string(),
        next_from,
        messages,
    })
}

pub(crate) async fn send_room_message_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    body: &str,
    formatted_body: Option<&str>,
) -> Result<String, String> {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return Err(String::from("Message cannot be empty"));
    }

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
        .map_err(|_| String::from("roomId is invalid"))?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let content = if let Some(formatted_body) = formatted_body.map(str::trim).filter(|value| !value.is_empty()) {
        RoomMessageEventContent::text_html(trimmed_body, formatted_body)
    } else {
        RoomMessageEventContent::text_plain(trimmed_body)
    };

    let response = room
        .send(content)
        .await
        .map_err(|error| format!("Failed to send room message: {error}"))?;

    Ok(response.event_id.to_string())
}

pub(crate) async fn toggle_reaction_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    target_event_id_raw: &str,
    key: &str,
) -> Result<(bool, Option<String>), String> {
    let reaction_key = key.trim();
    if reaction_key.is_empty() {
        return Err(String::from("Reaction key cannot be empty"));
    }

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
        .map_err(|_| String::from("roomId is invalid"))?;
    let target_event_id = matrix_sdk::ruma::OwnedEventId::try_from(target_event_id_raw)
        .map_err(|_| String::from("targetEventId is invalid"))?;

    let room = client
        .get_room(&room_id)
        .ok_or_else(|| String::from("Room is not available in current session"))?;

    let own_user_id = client
        .user_id()
        .ok_or_else(|| String::from("Session user ID is unavailable"))?
        .to_string();

    if let Some(existing_event_id) =
        find_matching_own_reaction_event_id(&room, &own_user_id, target_event_id_raw, reaction_key)
            .await?
    {
        let redact_target = matrix_sdk::ruma::OwnedEventId::try_from(existing_event_id.clone())
            .map_err(|_| String::from("Found invalid reaction event id for redaction"))?;

        room.redact(&redact_target, Some("Toggle reaction off"), None)
            .await
            .map_err(|error| format!("Failed to remove reaction: {error}"))?;

        return Ok((false, Some(existing_event_id)));
    }

    let content = ReactionEventContent::new(Annotation::new(target_event_id, reaction_key.to_owned()));
    let response = room
        .send(content)
        .await
        .map_err(|error| format!("Failed to send reaction: {error}"))?;

    Ok((true, Some(response.event_id.to_string())))
}

async fn find_matching_own_reaction_event_id(
    room: &matrix_sdk::Room,
    own_user_id: &str,
    target_event_id: &str,
    key: &str,
) -> Result<Option<String>, String> {
    let response = room
        .messages(build_messages_options(None, Some(100)))
        .await
        .map_err(|error| format!("Failed to read room messages for reaction toggle: {error}"))?;

    for timeline in response.chunk {
        let Ok(event) = timeline.raw().deserialize_as::<Value>() else {
            continue;
        };

        let Some(parsed) = parse_reaction_event(&event) else {
            continue;
        };

        if parsed.sender != own_user_id {
            continue;
        }

        if parsed.target_event_id != target_event_id {
            continue;
        }

        if parsed.key != key {
            continue;
        }

        if let Some(event_id) = parsed.event_id {
            return Ok(Some(event_id));
        }
    }

    Ok(None)
}

pub(crate) async fn load_picker_assets_from_client(
    client: &matrix_sdk::Client,
) -> Result<Vec<MatrixPickerCustomEmoji>, String> {
    let mut collected_custom_emoji = Vec::<MatrixPickerCustomEmoji>::new();
    let mut seen_custom_emoji = BTreeSet::<String>::new();
    let mut used_custom_emoji_names = BTreeSet::<String>::new();

    for room in client.joined_rooms() {
        for event_type in [
            "im.ponies.room_emotes",
            "org.matrix.msc2545.room_emotes",
            "im.ponies.room_packs",
            "org.matrix.msc2545.room_packs",
        ] {
            let fallback_usage = fallback_usage_from_event_type(event_type);
            let state_events = room
                .get_state_events(StateEventType::from(event_type))
                .await
                .map_err(|error| {
                    format!("Failed to load room emoji packs for {}: {error}", room.room_id())
                })?;

            for raw_event in state_events {
                let Ok(event) = serde_json::to_value(&raw_event) else {
                    continue;
                };

                let Some(content) = event.get("content") else {
                    continue;
                };

                merge_pack_content(
                    client,
                    content,
                    Some(room.room_id().to_string()),
                    fallback_usage,
                    true,
                    &mut collected_custom_emoji,
                    &mut seen_custom_emoji,
                    &mut used_custom_emoji_names,
                )
                .await;
            }
        }
    }

    for event_type in [
        "im.ponies.user_emotes",
        "org.matrix.msc2545.user_emotes",
        "im.ponies.user_packs",
        "org.matrix.msc2545.user_packs",
    ] {
        let fallback_usage = fallback_usage_from_event_type(event_type);
        let raw_content = client
            .account()
            .account_data_raw(GlobalAccountDataEventType::from(event_type))
            .await
            .map_err(|error| format!("Failed to load global emoji packs: {error}"))?;

        let Some(raw_content) = raw_content else {
            continue;
        };

        let Ok(content) = raw_content.deserialize_as::<Value>() else {
            continue;
        };

        merge_pack_content(
            client,
            &content,
            Some(String::from("Global")),
            fallback_usage,
            true,
            &mut collected_custom_emoji,
            &mut seen_custom_emoji,
            &mut used_custom_emoji_names,
        )
        .await;
    }

    Ok(collected_custom_emoji)
}

async fn merge_pack_content(
    client: &matrix_sdk::Client,
    content: &Value,
    fallback_category: Option<String>,
    fallback_usage: Option<&'static str>,
    resolve_references: bool,
    collected_custom_emoji: &mut Vec<MatrixPickerCustomEmoji>,
    seen_custom_emoji: &mut BTreeSet<String>,
    used_custom_emoji_names: &mut BTreeSet<String>,
) {
    let root_category = content
        .get("pack")
        .and_then(|pack| pack.get("display_name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or(fallback_category)
        .filter(|value| !value.trim().is_empty());

    if let Some(images) = content.get("images").and_then(Value::as_object) {
        merge_pack_images(
            client,
            content,
            images,
            root_category.clone(),
            fallback_usage,
            collected_custom_emoji,
            seen_custom_emoji,
            used_custom_emoji_names,
        )
        .await;
    }

    let Some(packs) = content.get("packs").and_then(Value::as_object) else {
        return;
    };

    for (pack_id, pack_content) in packs {
        let Some(images) = pack_content.get("images").and_then(Value::as_object) else {
            if resolve_references {
                merge_pack_reference(
                    client,
                    pack_content,
                    pack_id,
                    root_category.clone(),
                    fallback_usage,
                    collected_custom_emoji,
                    seen_custom_emoji,
                    used_custom_emoji_names,
                )
                .await;
            }
            continue;
        };

        let nested_category = pack_content
            .get("pack")
            .and_then(|pack| pack.get("display_name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                let trimmed = pack_id.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                }
            })
            .or(root_category.clone());

        merge_pack_images(
            client,
            pack_content,
            images,
            nested_category,
            fallback_usage,
            collected_custom_emoji,
            seen_custom_emoji,
            used_custom_emoji_names,
        )
        .await;
    }

    let Some(content_object) = content.as_object() else {
        return;
    };

    for (entry_key, entry_value) in content_object {
        if matches!(entry_key.as_str(), "pack" | "images" | "packs") {
            continue;
        }

        let Some(images) = entry_value.get("images").and_then(Value::as_object) else {
            continue;
        };

        let nested_category = entry_value
            .get("pack")
            .and_then(|pack| pack.get("display_name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                let trimmed = entry_key.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                }
            })
            .or(root_category.clone());

        merge_pack_images(
            client,
            entry_value,
            images,
            nested_category,
            fallback_usage,
            collected_custom_emoji,
            seen_custom_emoji,
            used_custom_emoji_names,
        )
        .await;
    }
}

async fn merge_pack_reference(
    client: &matrix_sdk::Client,
    pack_reference: &Value,
    pack_id: &str,
    root_category: Option<String>,
    fallback_usage: Option<&'static str>,
    collected_custom_emoji: &mut Vec<MatrixPickerCustomEmoji>,
    seen_custom_emoji: &mut BTreeSet<String>,
    used_custom_emoji_names: &mut BTreeSet<String>,
) {
    let Some(room_id_raw) = pack_reference.get("room_id").and_then(Value::as_str) else {
        return;
    };

    let Ok(room_id) = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw) else {
        return;
    };

    let Some(room) = client.get_room(&room_id) else {
        return;
    };

    let referenced_state_key = pack_reference.get("state_key").and_then(Value::as_str);

    let category = pack_reference
        .get("pack")
        .and_then(|pack| pack.get("display_name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let trimmed = pack_id.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        })
        .or(root_category);

    for event_type in [
        "im.ponies.room_emotes",
        "org.matrix.msc2545.room_emotes",
        "im.ponies.room_packs",
        "org.matrix.msc2545.room_packs",
    ] {
        let events = match room.get_state_events(StateEventType::from(event_type)).await {
            Ok(events) => events,
            Err(_) => continue,
        };

        for raw_event in events {
            let Ok(event) = serde_json::to_value(&raw_event) else {
                continue;
            };

            if let Some(expected_state_key) = referenced_state_key {
                let state_key = event
                    .get("state_key")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if state_key != expected_state_key {
                    continue;
                }
            }

            let Some(content) = event.get("content") else {
                continue;
            };

            merge_pack_content_non_recursive(
                client,
                content,
                category.clone(),
                fallback_usage_from_event_type(event_type).or(fallback_usage),
                collected_custom_emoji,
                seen_custom_emoji,
                used_custom_emoji_names,
            )
            .await;
        }
    }
}

async fn merge_pack_content_non_recursive(
    client: &matrix_sdk::Client,
    content: &Value,
    fallback_category: Option<String>,
    fallback_usage: Option<&'static str>,
    collected_custom_emoji: &mut Vec<MatrixPickerCustomEmoji>,
    seen_custom_emoji: &mut BTreeSet<String>,
    used_custom_emoji_names: &mut BTreeSet<String>,
) {
    let root_category = content
        .get("pack")
        .and_then(|pack| pack.get("display_name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or(fallback_category)
        .filter(|value| !value.trim().is_empty());

    if let Some(images) = content.get("images").and_then(Value::as_object) {
        merge_pack_images(
            client,
            content,
            images,
            root_category.clone(),
            fallback_usage,
            collected_custom_emoji,
            seen_custom_emoji,
            used_custom_emoji_names,
        )
        .await;
    }

    if let Some(packs) = content.get("packs").and_then(Value::as_object) {
        for (pack_id, pack_content) in packs {
            let Some(images) = pack_content.get("images").and_then(Value::as_object) else {
                continue;
            };

            let nested_category = pack_content
                .get("pack")
                .and_then(|pack| pack.get("display_name"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    let trimmed = pack_id.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_owned())
                    }
                })
                .or(root_category.clone());

            merge_pack_images(
                client,
                pack_content,
                images,
                nested_category,
                fallback_usage,
                collected_custom_emoji,
                seen_custom_emoji,
                used_custom_emoji_names,
            )
            .await;
        }
    }

    if let Some(content_object) = content.as_object() {
        for (entry_key, entry_value) in content_object {
            if matches!(entry_key.as_str(), "pack" | "images" | "packs") {
                continue;
            }

            let Some(images) = entry_value.get("images").and_then(Value::as_object) else {
                continue;
            };

            let nested_category = entry_value
                .get("pack")
                .and_then(|pack| pack.get("display_name"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    let trimmed = entry_key.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_owned())
                    }
                })
                .or(root_category.clone());

            merge_pack_images(
                client,
                entry_value,
                images,
                nested_category,
                fallback_usage,
                collected_custom_emoji,
                seen_custom_emoji,
                used_custom_emoji_names,
            )
            .await;
        }
    }
}

async fn merge_pack_images(
    client: &matrix_sdk::Client,
    usage_source: &Value,
    images: &serde_json::Map<String, Value>,
    category: Option<String>,
    fallback_usage: Option<&'static str>,
    collected_custom_emoji: &mut Vec<MatrixPickerCustomEmoji>,
    seen_custom_emoji: &mut BTreeSet<String>,
    used_custom_emoji_names: &mut BTreeSet<String>,
) {
    for (raw_shortcode, image) in images {
        let shortcode = raw_shortcode.trim_matches(':').trim();
        if shortcode.is_empty() {
            continue;
        }

        let Some(raw_url) = pack_media_url(image) else {
            continue;
        };

        let Some(url) = resolve_pack_media_url(client, raw_url).await else {
            continue;
        };

        let usage = image_usage(usage_source, image);
        let mut is_emoticon = usage_has_kind(&usage, "emoticon") || usage_has_kind(&usage, "emoji");

        if usage.is_empty() {
            match fallback_usage {
                Some("emoticon") => {
                    is_emoticon = true;
                }
                _ => {
                    is_emoticon = true;
                }
            }
        }

        let display_name = image
            .get("body")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| shortcode.to_owned());

        if is_emoticon {
            let dedupe_key = format!("{}|{}", shortcode.to_lowercase(), url);
            if !seen_custom_emoji.contains(&dedupe_key) {
                seen_custom_emoji.insert(dedupe_key);

                let name = unique_picker_name(used_custom_emoji_names, &display_name, shortcode);
                let source_url = canonical_pack_source_url(raw_url);
                collected_custom_emoji.push(MatrixPickerCustomEmoji {
                    name,
                    shortcodes: vec![shortcode.to_owned()],
                    url: url.clone(),
                    source_url,
                    category: category.clone(),
                });
            }
        }
    }
}

fn image_usage<'a>(content: &'a Value, image: &'a Value) -> BTreeSet<&'a str> {
    let mut usage = BTreeSet::new();

    if let Some(value) = image.get("usage").and_then(Value::as_str) {
        usage.insert(value);
    }

    if let Some(image_usage) = image.get("usage").and_then(Value::as_array) {
        for item in image_usage {
            if let Some(value) = item.as_str() {
                usage.insert(value);
            }
        }
    }

    if usage.is_empty() {
        if let Some(value) = content
            .get("pack")
            .and_then(|pack| pack.get("usage"))
            .and_then(Value::as_str)
        {
            usage.insert(value);
        }

        if let Some(pack_usage) = content
            .get("pack")
            .and_then(|pack| pack.get("usage"))
            .and_then(Value::as_array)
        {
            for item in pack_usage {
                if let Some(value) = item.as_str() {
                    usage.insert(value);
                }
            }
        }
    }

    usage
}

fn usage_has_kind(usage: &BTreeSet<&str>, kind: &str) -> bool {
    usage.iter().any(|entry| {
        let normalized = entry.trim().to_ascii_lowercase();
        normalized == kind
            || normalized.ends_with(&format!(".{kind}"))
            || normalized.contains(kind)
    })
}

fn fallback_usage_from_event_type(event_type: &str) -> Option<&'static str> {
    let normalized = event_type.to_ascii_lowercase();
    if normalized.contains("emote") {
        return Some("emoticon");
    }

    None
}

fn unique_picker_name(
    used_names: &mut BTreeSet<String>,
    display_name: &str,
    shortcode: &str,
) -> String {
    let trimmed = display_name.trim();
    let base = if trimmed.is_empty() { shortcode } else { trimmed };
    let mut candidate = if trimmed.is_empty() {
        shortcode.to_owned()
    } else {
        trimmed.to_owned()
    };

    if !used_names.contains(&candidate.to_lowercase()) {
        used_names.insert(candidate.to_lowercase());
        return candidate;
    }

    let mut suffix = 2_u32;
    loop {
        candidate = format!("{base}-{suffix}");
        let lower = candidate.to_lowercase();
        if !used_names.contains(&lower) {
            used_names.insert(lower);
            return candidate;
        }
        suffix = suffix.saturating_add(1);
    }
}

fn pack_media_url(image: &Value) -> Option<&str> {
    image
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| {
            image
                .get("file")
                .and_then(|value| value.get("url"))
                .and_then(Value::as_str)
        })
}

async fn resolve_pack_media_url(client: &matrix_sdk::Client, raw_url: &str) -> Option<String> {
    if raw_url.starts_with("mxc://") {
        if let Some(local) = cache_mxc_media_to_local_path(client, raw_url).await {
            return Some(local);
        }

        return matrix_media_url_from_event_url(&client.homeserver(), raw_url);
    }

    if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
        if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
            if let Some(local) = cache_mxc_media_to_local_path(client, &mxc_url).await {
                return Some(local);
            }

            if let Some(media_url) = matrix_media_url_from_event_url(&client.homeserver(), &mxc_url)
            {
                return Some(media_url);
            }
        }

        return Some(raw_url.to_owned());
    }

    matrix_media_url_from_event_url(&client.homeserver(), raw_url)
}

fn canonical_pack_source_url(raw_url: &str) -> String {
    if raw_url.starts_with("mxc://") {
        return raw_url.to_owned();
    }

    if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
        return mxc_url;
    }

    raw_url.to_owned()
}

fn mxc_from_matrix_media_download_url(raw_url: &str) -> Option<String> {
    let parsed = Url::parse(raw_url).ok()?;
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

fn matrix_media_url_from_event_url(homeserver_url: &Url, raw_url: &str) -> Option<String> {
    if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
        return Some(raw_url.to_owned());
    }

    if !raw_url.starts_with("mxc://") {
        return None;
    }

    let mxc = Url::parse(raw_url).ok()?;
    let server_name = mxc.host_str()?;
    let media_id = mxc.path().trim_start_matches('/');

    if media_id.is_empty() {
        return None;
    }

    let mut media_url = homeserver_url.clone();
    media_url.set_path(&format!(
        "/_matrix/media/v3/download/{server_name}/{media_id}"
    ));
    media_url.set_query(Some("allow_redirect=true"));
    media_url.set_fragment(None);

    Some(media_url.to_string())
}

fn build_messages_options(from: Option<String>, limit: Option<u32>) -> MessagesOptions {
    let mut options = MessagesOptions::new(Direction::Backward);
    options.from = from;
    options.limit = uint!(50);
    if let Some(limit) = limit {
        options.limit = limit.min(100).into();
    }
    options
}

async fn parse_message_chunk(
    client: &matrix_sdk::Client,
    chunk: Vec<TimelineEvent>,
) -> (Vec<MatrixChatMessage>, bool) {
    let mut messages = Vec::new();
    let mut had_utd = false;
    let mut reactions_by_target: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> =
        BTreeMap::new();

    for timeline in chunk {
        let encryption_info = timeline.encryption_info();
        let is_utd = timeline.kind.is_utd();
        let decryption_status = if is_utd {
            MatrixMessageDecryptionStatus::UnableToDecrypt
        } else if encryption_info.is_some() {
            MatrixMessageDecryptionStatus::Decrypted
        } else {
            MatrixMessageDecryptionStatus::Plaintext
        };

        if is_utd {
            had_utd = true;
        }

        let verification_status = match encryption_info.map(|info| &info.verification_state) {
            Some(VerificationState::Verified) => MatrixMessageVerificationStatus::Verified,
            Some(VerificationState::Unverified(_)) => MatrixMessageVerificationStatus::Unverified,
            None => MatrixMessageVerificationStatus::Unknown,
        };

        let Ok(event) = timeline.raw().deserialize_as::<Value>() else {
            continue;
        };

        if let Some(parsed_reaction) = parse_reaction_event(&event) {
            reactions_by_target
                .entry(parsed_reaction.target_event_id)
                .or_default()
                .entry(parsed_reaction.key)
                .or_default()
                .insert(parsed_reaction.sender);
            continue;
        }

        if let Some(parsed) = parse_timeline_message(
            &event,
            &client.homeserver(),
            decryption_status,
            verification_status,
        ) {
            let mut custom_emojis = Vec::with_capacity(parsed.custom_emojis.len());
            for emoji in parsed.custom_emojis {
                let resolved_url = resolve_pack_media_url(client, emoji.url.as_str())
                    .await
                    .unwrap_or(emoji.url);

                custom_emojis.push(MatrixCustomEmoji {
                    shortcode: emoji.shortcode,
                    url: resolved_url,
                });
            }

            let image_url = if matches!(
                parsed.message_type.as_deref(),
                Some("m.image")
            ) {
                resolve_image_cache_path(client, &event)
                    .await
                    .or(parsed.image_url)
            } else {
                parsed.image_url
            };

            messages.push(MatrixChatMessage {
                event_id: parsed.event_id,
                sender: parsed.sender,
                timestamp: parsed.timestamp,
                body: parsed.body,
                formatted_body: parsed.formatted_body,
                message_type: parsed.message_type,
                image_url,
                custom_emojis,
                reactions: Vec::new(),
                encrypted: parsed.encrypted,
                decryption_status: parsed.decryption_status,
                verification_status: parsed.verification_status,
            });
        }
    }

    if !reactions_by_target.is_empty() {
        for message in &mut messages {
            let Some(event_id) = &message.event_id else {
                continue;
            };

            let Some(reaction_map) = reactions_by_target.get(event_id) else {
                continue;
            };

            message.reactions = reaction_map
                .iter()
                .map(|(key, senders)| MatrixReactionSummary {
                    key: key.clone(),
                    count: senders.len() as u32,
                    senders: senders.iter().cloned().collect(),
                })
                .collect();
        }
    }

    (messages, had_utd)
}

async fn resolve_image_cache_path(client: &matrix_sdk::Client, event: &Value) -> Option<String> {
    let media_source = image_media_source_from_event(event)?;
    let mime_type = image_mime_type_from_event(event)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("application/octet-stream"));

    let request = MediaRequestParameters {
        source: media_source,
        format: MediaFormat::File,
    };

    let bytes = match client.media().get_media_content(&request, true).await {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!("Failed to fetch image media content: {error}");
            return None;
        }
    };

    persist_image_cache(&bytes, &mime_type, event)
}

fn persist_image_cache(bytes: &[u8], mime_type: &str, event: &Value) -> Option<String> {
    let extension = image_extension_from_mime(mime_type);
    let file_stem = image_cache_key(event, mime_type, bytes);
    persist_cached_media(bytes, &file_stem, extension, mime_type)
}

pub(crate) async fn cache_mxc_media_to_local_path(
    client: &matrix_sdk::Client,
    raw_url: &str,
) -> Option<String> {
    if !raw_url.starts_with("mxc://") {
        return None;
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

    persist_cached_media(&bytes, &file_stem, extension, mime_type)
}

fn persist_cached_media(
    bytes: &[u8],
    file_stem: &str,
    extension: &str,
    mime_type: &str,
) -> Option<String> {
    if matches!(media_storage_mode(), MediaStorageMode::InMemory) {
        return persist_cached_media_in_memory(bytes, file_stem, extension, mime_type);
    }

    persist_cached_media_asset(bytes, file_stem, extension)
}

fn persist_cached_media_in_memory(
    bytes: &[u8],
    file_stem: &str,
    extension: &str,
    mime_type: &str,
) -> Option<String> {
    let media_key = format!("{file_stem}.{extension}");

    let cache = in_memory_media_cache();
    let mut lock = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return None,
    };

    lock.insert(
        media_key.clone(),
        InMemoryMediaValue {
            bytes: bytes.to_vec(),
            mime_type: mime_type.to_owned(),
        },
    );

    Some(to_in_memory_media_url(&media_key))
}

fn persist_cached_media_asset(bytes: &[u8], file_stem: &str, extension: &str) -> Option<String> {
    let cache_dir = media_cache_dir();
    if let Err(error) = fs::create_dir_all(&cache_dir) {
        warn!("Failed to initialize media cache directory: {error}");
        return None;
    }

    let file_name = format!("{file_stem}.{extension}");
    let final_path = cache_dir.join(file_name);

    if final_path.exists() {
        return Some(final_path.to_string_lossy().to_string());
    }

    let temp_path = cache_dir.join(format!("{file_stem}.tmp"));
    if let Err(error) = fs::write(&temp_path, bytes) {
        warn!("Failed to write cached media file: {error}");
        return None;
    }

    if let Err(error) = fs::rename(&temp_path, &final_path) {
        let _ = fs::remove_file(&temp_path);
        if final_path.exists() {
            return Some(final_path.to_string_lossy().to_string());
        }
        warn!("Failed to finalize cached media file: {error}");
        return None;
    }

    Some(final_path.to_string_lossy().to_string())
}

fn mxc_image_cache_key(raw_url: &str, bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    raw_url.hash(&mut hasher);
    bytes.len().hash(&mut hasher);
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

fn image_extension_from_mime(mime_type: &str) -> &'static str {
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

fn image_cache_key(event: &Value, mime_type: &str, bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    event
        .get("event_id")
        .and_then(Value::as_str)
        .hash(&mut hasher);
    event
        .get("origin_server_ts")
        .and_then(Value::as_u64)
        .hash(&mut hasher);
    event
        .get("room_id")
        .and_then(Value::as_str)
        .hash(&mut hasher);
    image_source_key(event).hash(&mut hasher);
    mime_type.hash(&mut hasher);

    // Include content size so messages without event_id still get stable keys per content.
    bytes.len().hash(&mut hasher);

    format!("img-{:016x}", hasher.finish())
}

fn image_source_key(event: &Value) -> Option<&str> {
    event
        .get("content")
        .and_then(|content| content.get("url"))
        .and_then(Value::as_str)
        .or_else(|| {
            event
                .get("content")
                .and_then(|content| content.get("file"))
                .and_then(|file| file.get("url"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            event
                .get("content")
                .and_then(|content| content.get("info"))
                .and_then(|info| info.get("thumbnail_url"))
                .and_then(Value::as_str)
        })
}

fn image_media_source_from_event(event: &Value) -> Option<MediaSource> {
    let content = event.get("content")?;
    serde_json::from_value(content.clone()).ok()
}

fn image_mime_type_from_event(event: &Value) -> Option<String> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("mimetype"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}
