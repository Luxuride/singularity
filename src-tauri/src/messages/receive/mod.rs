use std::collections::{BTreeMap, BTreeSet, HashMap};

use log::{debug, warn};
use matrix_sdk::deserialized_responses::{TimelineEvent, VerificationState};
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::uint;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::db::AppDb;
use crate::protocol::event_paths;
use crate::protocol::events_schema::{parse_reaction_event, parse_timeline_message};
use crate::rooms::{RoomRefreshTrigger, RoomUpdateTriggerState};

use super::media::{DefaultMediaResolver, MediaResolver};
use super::persistence::{
    is_cacheable_initial_request, load_initial_room_messages, store_initial_room_messages,
};
use super::types::{
    MatrixChatMessage, MatrixChatMessageStreamEvent, MatrixCustomEmoji,
    MatrixGetChatMessagesResponse, MatrixMessageDecryptionStatus, MatrixMessageLoadKind,
    MatrixMessageVerificationStatus, MatrixReactionSummary, MatrixStreamChatMessagesRequest,
    MatrixStreamChatMessagesResponse,
};

#[derive(Clone, Copy)]
pub(crate) struct StreamRoomMessagesContext<'a> {
    pub(crate) app_handle: &'a AppHandle,
    pub(crate) app_db: &'a AppDb,
    pub(crate) room_update_trigger_state: &'a RoomUpdateTriggerState,
    pub(crate) client: &'a matrix_sdk::Client,
}

pub(crate) trait MessageReceiver {
    async fn fetch_room_messages(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        from: Option<String>,
        limit: Option<u32>,
    ) -> Result<MatrixGetChatMessagesResponse, String>;

    async fn stream_room_messages(
        &self,
        context: StreamRoomMessagesContext<'_>,
        request: MatrixStreamChatMessagesRequest,
    ) -> Result<MatrixStreamChatMessagesResponse, String>;
}

pub(crate) struct MatrixMessageReceiver<M: MediaResolver = DefaultMediaResolver> {
    media_resolver: M,
}

impl Default for MatrixMessageReceiver<DefaultMediaResolver> {
    fn default() -> Self {
        Self {
            media_resolver: DefaultMediaResolver,
        }
    }
}

impl<M: MediaResolver> MatrixMessageReceiver<M> {
    async fn parse_message_chunk(
        &self,
        client: &matrix_sdk::Client,
        chunk: Vec<TimelineEvent>,
    ) -> (Vec<MatrixChatMessage>, bool) {
        let mut messages = Vec::new();
        let mut had_utd = false;
        let mut resolved_emoji_urls: HashMap<String, Option<String>> = HashMap::new();
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
                Some(VerificationState::Unverified(_)) => {
                    MatrixMessageVerificationStatus::Unverified
                }
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
                    let resolved_url = match resolved_emoji_urls.get(emoji.url.as_str()) {
                        Some(cached) => cached.clone(),
                        None => {
                            let resolved = self
                                .media_resolver
                                .resolve_pack_media_url(client, emoji.url.as_str())
                                .await;
                            resolved_emoji_urls.insert(emoji.url.clone(), resolved.clone());
                            resolved
                        }
                    };

                    let Some(resolved_url) = resolved_url else {
                        continue;
                    };

                    custom_emojis.push(MatrixCustomEmoji {
                        shortcode: emoji.shortcode,
                        url: resolved_url,
                    });
                }

                let image_url = if matches!(parsed.message_type.as_deref(), Some("m.image")) {
                    self.media_resolver
                        .resolve_image_cache_path(client, &event)
                        .await
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
}

impl<M: MediaResolver> MessageReceiver for MatrixMessageReceiver<M> {
    async fn fetch_room_messages(
        &self,
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

        let (mut messages, mut had_utd) = self.parse_message_chunk(client, response.chunk).await;
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
            } else if let Ok(retry_response) =
                room.messages(build_messages_options(from, limit)).await
            {
                let (retry_messages, retry_had_utd) =
                    self.parse_message_chunk(client, retry_response.chunk).await;
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

    async fn stream_room_messages(
        &self,
        context: StreamRoomMessagesContext<'_>,
        request: MatrixStreamChatMessagesRequest,
    ) -> Result<MatrixStreamChatMessagesResponse, String> {
        let StreamRoomMessagesContext {
            app_handle,
            app_db,
            room_update_trigger_state,
            client,
        } = context;
        let MatrixStreamChatMessagesRequest {
            room_id,
            from,
            limit,
            stream_id,
            load_kind,
        } = request;

        let target_message_count = limit.unwrap_or(50).clamp(1, 100) as usize;
        let cacheable_initial_request = matches!(load_kind, MatrixMessageLoadKind::Initial)
            && is_cacheable_initial_request(from.as_deref(), limit);

        if cacheable_initial_request {
            if let Some(cached) =
                load_initial_room_messages(app_db, room_id.as_str(), from.as_deref(), limit)?
            {
                let mut cached_messages = cached.messages;
                if matches!(load_kind, MatrixMessageLoadKind::Initial) {
                    cached_messages.reverse();
                }
                let mut sequence = 0_u32;

                for message in cached_messages {
                    app_handle
                        .emit(
                            event_paths::CHAT_MESSAGES_STREAM,
                            MatrixChatMessageStreamEvent {
                                room_id: room_id.clone(),
                                stream_id: stream_id.clone(),
                                load_kind,
                                sequence,
                                message: Some(message),
                                next_from: None,
                                done: false,
                            },
                        )
                        .map_err(|error| {
                            format!("Failed to emit chat message stream event: {error}")
                        })?;

                    sequence = sequence.saturating_add(1);
                }

                app_handle
                    .emit(
                        event_paths::CHAT_MESSAGES_STREAM,
                        MatrixChatMessageStreamEvent {
                            room_id: room_id.clone(),
                            stream_id: stream_id.clone(),
                            load_kind,
                            sequence,
                            message: None,
                            next_from: cached.next_from,
                            done: true,
                        },
                    )
                    .map_err(|error| {
                        format!("Failed to emit chat message stream completion: {error}")
                    })?;

                let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
                    selected_room_id: Some(room_id),
                    include_selected_messages: false,
                });

                return Ok(MatrixStreamChatMessagesResponse {
                    stream_id,
                    started: true,
                });
            }
        }

        let mut scan_from = from;
        let mut cache_messages = Vec::with_capacity(target_message_count);
        let mut final_next_from = None;
        let mut request_count = 0_usize;
        let mut same_cursor_count = 0_usize;
        let max_request_count = ((target_message_count.saturating_add(49)) / 50)
            .saturating_mul(6)
            .max(8);
        let mut sequence = 0_u32;

        while sequence < target_message_count as u32 && request_count < max_request_count {
            let remaining = target_message_count.saturating_sub(sequence as usize);
            let batch_limit = remaining.min(50) as u32;
            let previous_scan_from = scan_from.clone();

            let response = self
                .fetch_room_messages(
                    client,
                    room_id.as_str(),
                    scan_from.clone(),
                    Some(batch_limit),
                )
                .await?;

            request_count = request_count.saturating_add(1);
            final_next_from = response.next_from.clone();
            let message_count = response.messages.len();
            scan_from = response.next_from;

            if scan_from == previous_scan_from {
                same_cursor_count = same_cursor_count.saturating_add(1);
            } else {
                same_cursor_count = 0;
            }

            let mut batch_messages = response.messages;
            if matches!(load_kind, MatrixMessageLoadKind::Initial) {
                batch_messages.reverse();
            }

            for message in batch_messages {
                if sequence >= target_message_count as u32 {
                    break;
                }

                if cacheable_initial_request {
                    cache_messages.push(message.clone());
                }

                app_handle
                    .emit(
                        event_paths::CHAT_MESSAGES_STREAM,
                        MatrixChatMessageStreamEvent {
                            room_id: room_id.clone(),
                            stream_id: stream_id.clone(),
                            load_kind,
                            sequence,
                            message: Some(message),
                            next_from: None,
                            done: false,
                        },
                    )
                    .map_err(|error| {
                        format!("Failed to emit chat message stream event: {error}")
                    })?;

                sequence = sequence.saturating_add(1);
            }

            if scan_from.is_none() {
                break;
            }

            if message_count == 0 {
                break;
            }

            if same_cursor_count >= 2 {
                break;
            }
        }

        let next_from = final_next_from.clone();

        if cacheable_initial_request {
            store_initial_room_messages(
                app_db,
                &MatrixGetChatMessagesResponse {
                    room_id: room_id.clone(),
                    next_from: next_from.clone(),
                    messages: cache_messages,
                },
            )?;
        }

        app_handle
            .emit(
                event_paths::CHAT_MESSAGES_STREAM,
                MatrixChatMessageStreamEvent {
                    room_id,
                    stream_id: stream_id.clone(),
                    load_kind,
                    sequence,
                    message: None,
                    next_from,
                    done: true,
                },
            )
            .map_err(|error| format!("Failed to emit chat message stream completion: {error}"))?;

        Ok(MatrixStreamChatMessagesResponse {
            stream_id,
            started: true,
        })
    }
}

pub(crate) async fn fetch_room_messages_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    from: Option<String>,
    limit: Option<u32>,
) -> Result<MatrixGetChatMessagesResponse, String> {
    MatrixMessageReceiver::default()
        .fetch_room_messages(client, room_id_raw, from, limit)
        .await
}

pub(crate) async fn stream_room_messages_from_client(
    context: StreamRoomMessagesContext<'_>,
    request: MatrixStreamChatMessagesRequest,
) -> Result<MatrixStreamChatMessagesResponse, String> {
    MatrixMessageReceiver::default()
        .stream_room_messages(context, request)
        .await
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
