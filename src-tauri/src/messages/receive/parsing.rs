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
            let mut custom_emoji_urls_by_source = HashMap::<String, String>::new();
            for emoji in parsed.custom_emojis {
                let source_url = emoji.url.clone();
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

                if source_url.is_empty() || custom_emoji_urls_by_source.contains_key(&source_url) {
                    continue;
                }

                custom_emoji_urls_by_source.insert(
                    source_url,
                    custom_emojis
                        .last()
                        .expect("custom emoji just pushed")
                        .url
                        .clone(),
                );
            }

            let formatted_body = parsed.formatted_body.map(|body| {
                rewrite_formatted_body_custom_emoji(&body, &custom_emoji_urls_by_source)
            });

            let image_url = if matches!(parsed.message_type.as_deref(), Some("m.image")) {
                media_resolver
                    .resolve_image_cache_path(client, &event)
                    .await
            } else {
                parsed.image_url
            };

            messages.push(MatrixChatMessage {
                event_id: parsed.event_id,
                in_reply_to_event_id: parsed.in_reply_to_event_id,
                sender: parsed.sender,
                timestamp: parsed.timestamp,
                body: parsed.body,
                formatted_body,
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

fn rewrite_formatted_body_custom_emoji(
    formatted_body: &str,
    custom_emoji_urls_by_source: &HashMap<String, String>,
) -> String {
    if custom_emoji_urls_by_source.is_empty() {
        return formatted_body.to_owned();
    }

    let mut rewritten = String::with_capacity(formatted_body.len());
    let mut search_index = 0_usize;

    while let Some(relative_start) = formatted_body[search_index..].find("<img") {
        let tag_start = search_index + relative_start;
        let Some(relative_end) = formatted_body[tag_start..].find('>') else {
            rewritten.push_str(&formatted_body[search_index..]);
            return rewritten;
        };

        let tag_end = tag_start + relative_end + 1;
        let tag = &formatted_body[tag_start..tag_end];
        rewritten.push_str(&formatted_body[search_index..tag_start]);
        rewritten.push_str(&rewrite_img_tag(tag, custom_emoji_urls_by_source));
        search_index = tag_end;
    }

    rewritten.push_str(&formatted_body[search_index..]);
    rewritten
}

fn rewrite_img_tag(tag: &str, custom_emoji_urls_by_source: &HashMap<String, String>) -> String {
    // Rewrite src URL for all img elements
    let mut result = tag.to_owned();
    if let Some(source_url) = extract_html_attribute(tag, "src") {
        if let Some(rewritten_src) = custom_emoji_urls_by_source.get(&source_url) {
            result = replace_html_attribute(&result, "src", rewritten_src);
        }
    }

    // Handle width/height dimensions only for emoji
    let is_emoji = result.contains("data-mx-emoticon") || result.contains("data_mx_emoticon");
    if is_emoji {
        let width = extract_html_attribute(&result, "width");
        let height = extract_html_attribute(&result, "height");

        match (width, height) {
            (None, None) => {
                result = add_html_attribute(&result, "width", "32");
                result = add_html_attribute(&result, "height", "32");
            }
            (None, Some(h)) => {
                result = add_html_attribute(&result, "width", &h);
            }
            (Some(w), None) => {
                result = add_html_attribute(&result, "height", &w);
            }
            (Some(_), Some(_)) => {
                // Both present: no change
            }
        }
    }

    result
}

fn extract_html_attribute(tag: &str, attribute: &str) -> Option<String> {
    let quoted_pattern = format!("{attribute}=\"");
    if let Some(start_index) = tag.find(&quoted_pattern) {
        let value_start = start_index + quoted_pattern.len();
        let value_end = tag[value_start..].find('"')? + value_start;
        return Some(tag[value_start..value_end].to_owned());
    }

    let single_quote_pattern = format!("{attribute}='");
    if let Some(start_index) = tag.find(&single_quote_pattern) {
        let value_start = start_index + single_quote_pattern.len();
        let value_end = tag[value_start..].find('\'')? + value_start;
        return Some(tag[value_start..value_end].to_owned());
    }

    None
}

fn replace_html_attribute(tag: &str, attribute: &str, value: &str) -> String {
    let quoted_pattern = format!("{attribute}=\"");
    if let Some(start_index) = tag.find(&quoted_pattern) {
        let value_start = start_index + quoted_pattern.len();
        if let Some(relative_end) = tag[value_start..].find('"') {
            let value_end = value_start + relative_end;
            return format!("{}{}{}", &tag[..value_start], value, &tag[value_end..]);
        }
    }

    let single_quote_pattern = format!("{attribute}='");
    if let Some(start_index) = tag.find(&single_quote_pattern) {
        let value_start = start_index + single_quote_pattern.len();
        if let Some(relative_end) = tag[value_start..].find('\'') {
            let value_end = value_start + relative_end;
            return format!("{}{}{}", &tag[..value_start], value, &tag[value_end..]);
        }
    }

    tag.to_owned()
}

fn add_html_attribute(tag: &str, attribute: &str, value: &str) -> String {
    // First try to replace if it exists
    let quoted_pattern = format!("{attribute}=\"");
    if tag.contains(&quoted_pattern) {
        return replace_html_attribute(tag, attribute, value);
    }

    let single_quote_pattern = format!("{attribute}='");
    if tag.contains(&single_quote_pattern) {
        return replace_html_attribute(tag, attribute, value);
    }

    // Attribute doesn't exist, add it before the closing >
    if let Some(close_pos) = tag.rfind('>') {
        let before_close = &tag[..close_pos];
        let close_char = &tag[close_pos..];
        format!("{} {}=\"{}\"{}", before_close, attribute, value, close_char)
    } else {
        tag.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::rewrite_formatted_body_custom_emoji;

    #[test]
    fn rewrites_custom_emoji_image_src_to_matrix_media() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/wave".to_owned(),
            "matrix-media://localhost/wave".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p>hello <img data-mx-emoticon src=\"mxc://media.example.org/wave\" alt=\":wave:\"></p>",
            &custom_emoji_urls_by_source,
        );

        assert_eq!(
            rewritten,
            "<p>hello <img data-mx-emoticon src=\"matrix-media://localhost/wave\" alt=\":wave:\" width=\"32\" height=\"32\"></p>"
        );
    }

    #[test]
    fn adds_dimensions_when_missing() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/smile".to_owned(),
            "matrix-media://localhost/smile".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p><img data-mx-emoticon src=\"mxc://media.example.org/smile\" alt=\":smile:\"></p>",
            &custom_emoji_urls_by_source,
        );

        assert!(rewritten.contains("width=\"32\""));
        assert!(rewritten.contains("height=\"32\""));
    }

    #[test]
    fn sets_width_equal_to_height_when_width_missing() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/joy".to_owned(),
            "matrix-media://localhost/joy".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p><img data-mx-emoticon src=\"mxc://media.example.org/joy\" alt=\":joy:\" height=\"48\"></p>",
            &custom_emoji_urls_by_source,
        );

        assert!(rewritten.contains("width=\"48\""));
        assert!(rewritten.contains("height=\"48\""));
    }

    #[test]
    fn sets_height_equal_to_width_when_height_missing() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/sad".to_owned(),
            "matrix-media://localhost/sad".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p><img data-mx-emoticon src=\"mxc://media.example.org/sad\" alt=\":sad:\" width=\"64\"></p>",
            &custom_emoji_urls_by_source,
        );

        assert!(rewritten.contains("width=\"64\""));
        assert!(rewritten.contains("height=\"64\""));
    }

    #[test]
    fn handles_underscore_data_mx_emoticon_format() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://matrix.luxuride.eu/GPXPlREYzJTkwoPxjpTkfMWg".to_owned(),
            "matrix-media://localhost/GPXPlREYzJTkwoPxjpTkfMWg".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p>Test <img data_mx_emoticon=\"\" src=\"mxc://matrix.luxuride.eu/GPXPlREYzJTkwoPxjpTkfMWg\" alt=\":neuro-flushed:\" title=\":neuro-flushed:\" height=\"32\"></img></p>",
            &custom_emoji_urls_by_source,
        );

        assert!(rewritten.contains("matrix-media://localhost/GPXPlREYzJTkwoPxjpTkfMWg"));
        assert!(!rewritten.contains("mxc://matrix.luxuride.eu"));
        assert!(rewritten.contains("width=\"32\""));
        assert!(rewritten.contains("height=\"32\""));
    }

    #[test]
    fn rewrites_non_emoji_img_urls_when_in_map() {
        let mut custom_emoji_urls_by_source = HashMap::new();
        custom_emoji_urls_by_source.insert(
            "mxc://media.example.org/image1".to_owned(),
            "matrix-media://localhost/image1".to_owned(),
        );

        let rewritten = rewrite_formatted_body_custom_emoji(
            "<p>Here is an image: <img src=\"mxc://media.example.org/image1\" alt=\"Some image\"></p>",
            &custom_emoji_urls_by_source,
        );

        // Should rewrite URL since it's in the map
        assert!(rewritten.contains("matrix-media://localhost/image1"));

        // Should NOT add dimensions (not an emoji)
        assert!(!rewritten.contains("width=\"32\""));
        assert!(!rewritten.contains("height=\"32\""));
    }
}
