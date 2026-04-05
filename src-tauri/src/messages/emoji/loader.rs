use std::collections::BTreeSet;

use matrix_sdk::ruma::events::{GlobalAccountDataEventType, StateEventType};
use serde_json::Value;

use super::super::media::{DefaultMediaResolver, MediaResolver};
use super::super::types::MatrixPickerCustomEmoji;
use super::category::{
    nested_category_from_entry, nested_category_from_pack, referenced_category, root_category,
};
use super::constants::{ROOM_EMOJI_EVENT_TYPES, USER_EMOJI_EVENT_TYPES};
use super::pack_parsing::{
    fallback_usage_from_event_type, image_usage, pack_media_url, unique_picker_name, usage_has_kind,
};

pub(crate) trait EmojiLoader {
    async fn load_picker_assets(
        &self,
        client: &matrix_sdk::Client,
    ) -> Result<Vec<MatrixPickerCustomEmoji>, String>;
}

#[derive(Default, Clone)]
pub(crate) struct MatrixEmojiLoader<M: MediaResolver = DefaultMediaResolver> {
    media_resolver: M,
}

impl<M: MediaResolver> EmojiLoader for MatrixEmojiLoader<M> {
    async fn load_picker_assets(
        &self,
        client: &matrix_sdk::Client,
    ) -> Result<Vec<MatrixPickerCustomEmoji>, String> {
        let mut accumulator = EmojiPackAccumulator {
            collected_custom_emoji: Vec::new(),
            seen_custom_emoji: BTreeSet::new(),
            used_custom_emoji_names: BTreeSet::new(),
        };

        for room in client.joined_rooms() {
            for event_type in ROOM_EMOJI_EVENT_TYPES {
                let fallback_usage = fallback_usage_from_event_type(event_type);
                let state_events = room
                    .get_state_events(StateEventType::from(event_type))
                    .await
                    .map_err(|error| {
                        format!(
                            "Failed to load room emoji packs for {}: {error}",
                            room.room_id()
                        )
                    })?;

                for raw_event in state_events {
                    let Ok(event) = serde_json::to_value(&raw_event) else {
                        continue;
                    };

                    let Some(content) = event.get("content") else {
                        continue;
                    };

                    self.merge_pack_content(
                        client,
                        content,
                        Some(room.room_id().to_string()),
                        fallback_usage,
                        true,
                        &mut accumulator,
                    )
                    .await;
                }
            }
        }

        for event_type in USER_EMOJI_EVENT_TYPES {
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

            self.merge_pack_content(
                client,
                &content,
                Some(String::from("Global")),
                fallback_usage,
                true,
                &mut accumulator,
            )
            .await;
        }

        Ok(accumulator.collected_custom_emoji)
    }
}

struct EmojiPackAccumulator {
    collected_custom_emoji: Vec<MatrixPickerCustomEmoji>,
    seen_custom_emoji: BTreeSet<String>,
    used_custom_emoji_names: BTreeSet<String>,
}

impl<M: MediaResolver> MatrixEmojiLoader<M> {
    async fn merge_pack_content(
        &self,
        client: &matrix_sdk::Client,
        content: &Value,
        fallback_category: Option<String>,
        fallback_usage: Option<&'static str>,
        resolve_references: bool,
        accumulator: &mut EmojiPackAccumulator,
    ) {
        let root_category = root_category(content, fallback_category);

        if let Some(images) = content.get("images").and_then(Value::as_object) {
            self.merge_pack_images(
                client,
                content,
                images,
                root_category.clone(),
                fallback_usage,
                accumulator,
            )
            .await;
        }

        let Some(packs) = content.get("packs").and_then(Value::as_object) else {
            return;
        };

        for (pack_id, pack_content) in packs {
            let Some(images) = pack_content.get("images").and_then(Value::as_object) else {
                if resolve_references {
                    self.merge_pack_reference(
                        client,
                        pack_content,
                        pack_id,
                        root_category.clone(),
                        fallback_usage,
                        accumulator,
                    )
                    .await;
                }
                continue;
            };

            let nested_category =
                nested_category_from_pack(pack_content, pack_id, root_category.clone());

            self.merge_pack_images(
                client,
                pack_content,
                images,
                nested_category,
                fallback_usage,
                accumulator,
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

            let nested_category =
                nested_category_from_entry(entry_value, entry_key, root_category.clone());

            self.merge_pack_images(
                client,
                entry_value,
                images,
                nested_category,
                fallback_usage,
                accumulator,
            )
            .await;
        }
    }

    async fn merge_pack_reference(
        &self,
        client: &matrix_sdk::Client,
        pack_reference: &Value,
        pack_id: &str,
        root_category: Option<String>,
        fallback_usage: Option<&'static str>,
        accumulator: &mut EmojiPackAccumulator,
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

        let category = referenced_category(pack_reference, pack_id, root_category);

        for event_type in ROOM_EMOJI_EVENT_TYPES {
            let events = match room
                .get_state_events(StateEventType::from(event_type))
                .await
            {
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

                self.merge_pack_content_non_recursive(
                    client,
                    content,
                    category.clone(),
                    fallback_usage_from_event_type(event_type).or(fallback_usage),
                    accumulator,
                )
                .await;
            }
        }
    }

    async fn merge_pack_content_non_recursive(
        &self,
        client: &matrix_sdk::Client,
        content: &Value,
        fallback_category: Option<String>,
        fallback_usage: Option<&'static str>,
        accumulator: &mut EmojiPackAccumulator,
    ) {
        let root_category = root_category(content, fallback_category);

        if let Some(images) = content.get("images").and_then(Value::as_object) {
            self.merge_pack_images(
                client,
                content,
                images,
                root_category.clone(),
                fallback_usage,
                accumulator,
            )
            .await;
        }

        if let Some(packs) = content.get("packs").and_then(Value::as_object) {
            for (pack_id, pack_content) in packs {
                let Some(images) = pack_content.get("images").and_then(Value::as_object) else {
                    continue;
                };

                let nested_category =
                    nested_category_from_pack(pack_content, pack_id, root_category.clone());

                self.merge_pack_images(
                    client,
                    pack_content,
                    images,
                    nested_category,
                    fallback_usage,
                    accumulator,
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

                let nested_category =
                    nested_category_from_entry(entry_value, entry_key, root_category.clone());

                self.merge_pack_images(
                    client,
                    entry_value,
                    images,
                    nested_category,
                    fallback_usage,
                    accumulator,
                )
                .await;
            }
        }
    }

    async fn merge_pack_images(
        &self,
        client: &matrix_sdk::Client,
        usage_source: &Value,
        images: &serde_json::Map<String, Value>,
        category: Option<String>,
        fallback_usage: Option<&'static str>,
        accumulator: &mut EmojiPackAccumulator,
    ) {
        for (raw_shortcode, image) in images {
            let shortcode = raw_shortcode.trim_matches(':').trim();
            if shortcode.is_empty() {
                continue;
            }

            let Some(raw_url) = pack_media_url(image) else {
                continue;
            };

            let Some(url) = self
                .media_resolver
                .resolve_pack_media_url(client, raw_url)
                .await
            else {
                continue;
            };

            let usage = image_usage(usage_source, image);
            let mut is_emoticon =
                usage_has_kind(&usage, "emoticon") || usage_has_kind(&usage, "emoji");

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
                if !accumulator.seen_custom_emoji.contains(&dedupe_key) {
                    accumulator.seen_custom_emoji.insert(dedupe_key);

                    let name = unique_picker_name(
                        &mut accumulator.used_custom_emoji_names,
                        &display_name,
                        shortcode,
                    );
                    let source_url = self.media_resolver.canonical_pack_source_url(raw_url);
                    accumulator
                        .collected_custom_emoji
                        .push(MatrixPickerCustomEmoji {
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
}

pub(crate) async fn load_picker_assets_from_client(
    client: &matrix_sdk::Client,
) -> Result<Vec<MatrixPickerCustomEmoji>, String> {
    MatrixEmojiLoader::<DefaultMediaResolver>::default()
        .load_picker_assets(client)
        .await
}
