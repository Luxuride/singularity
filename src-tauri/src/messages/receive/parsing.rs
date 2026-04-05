use std::collections::{BTreeMap, BTreeSet, HashMap};

use matrix_sdk::deserialized_responses::{TimelineEvent, VerificationState};
use serde_json::Value;

use crate::protocol::events_schema::{parse_reaction_event, parse_timeline_message};

use super::super::media::MediaResolver;
use super::super::types::{
    MatrixChatMessage, MatrixCustomEmoji, MatrixMessageDecryptionStatus,
    MatrixMessageVerificationStatus, MatrixReactionSummary,
};

pub(super) async fn parse_message_chunk<M: MediaResolver>(
    media_resolver: &M,
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
                let resolved_url = match resolved_emoji_urls.get(emoji.url.as_str()) {
                    Some(cached) => cached.clone(),
                    None => {
                        let resolved = media_resolver
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
                media_resolver
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
