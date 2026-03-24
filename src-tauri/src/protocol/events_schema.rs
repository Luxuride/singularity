use serde_json::Value;
use url::Url;

use crate::messages::{MatrixMessageDecryptionStatus, MatrixMessageVerificationStatus};
use crate::protocol::event_types;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedTimelineMessage {
    pub event_id: Option<String>,
    pub sender: String,
    pub timestamp: Option<u64>,
    pub body: String,
    pub formatted_body: Option<String>,
    pub message_type: Option<String>,
    pub image_url: Option<String>,
    pub custom_emojis: Vec<ParsedCustomEmoji>,
    pub encrypted: bool,
    pub decryption_status: MatrixMessageDecryptionStatus,
    pub verification_status: MatrixMessageVerificationStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCustomEmoji {
    pub shortcode: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedReactionEvent {
    pub event_id: Option<String>,
    pub sender: String,
    pub target_event_id: String,
    pub key: String,
}

pub fn parse_reaction_event(event: &Value) -> Option<ParsedReactionEvent> {
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if event_type != event_types::REACTION {
        return None;
    }

    let sender = event
        .get("sender")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let event_id = event
        .get("event_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    let relates_to = event
        .get("content")
        .and_then(|value| value.get("m.relates_to"))?;
    let rel_type = relates_to
        .get("rel_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if rel_type != "m.annotation" {
        return None;
    }

    let target_event_id = relates_to
        .get("event_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let key = relates_to
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if target_event_id.is_empty() || key.is_empty() {
        return None;
    }

    Some(ParsedReactionEvent {
        event_id,
        sender,
        target_event_id: target_event_id.to_owned(),
        key: key.to_owned(),
    })
}

pub fn parse_timeline_message(
    event: &Value,
    homeserver_url: &Url,
    decryption_status: MatrixMessageDecryptionStatus,
    verification_status: MatrixMessageVerificationStatus,
) -> Option<ParsedTimelineMessage> {
    let event_type = event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let sender = event
        .get("sender")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let event_id = event
        .get("event_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let timestamp = event.get("origin_server_ts").and_then(Value::as_u64);

    if event_type == event_types::ROOM_MESSAGE {
        let content = event.get("content");
        let msgtype = event
            .get("content")
            .and_then(|content| content.get("msgtype"))
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(event_type);
        let message_type = if msgtype.is_empty() {
            None
        } else {
            Some(msgtype.to_owned())
        };
        let body = content
            .and_then(|content| content.get("body"))
            .and_then(Value::as_str)
            .unwrap_or("Unsupported message")
            .to_owned();
        let formatted_body = content
            .and_then(|content| content.get("formatted_body"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let custom_emojis = formatted_body
            .as_deref()
            .map(parse_custom_emojis_from_formatted_body)
            .unwrap_or_default();

        let text_body = if msgtype == event_types::message_types::TEXT
            || msgtype == event_types::message_types::NOTICE
            || msgtype == event_types::message_types::EMOTE
        {
            Some((body, None))
        } else if msgtype == event_types::message_types::IMAGE {
            let image_url = extract_media_event_url(event)
                .and_then(|raw| matrix_media_url_from_event_url(homeserver_url, raw));

            Some((body, image_url))
        } else {
            None
        };

        let (body, image_url) = match text_body {
            Some(values) => values,
            None => (format!("Unsupported message type: {msgtype}"), None),
        };

        return Some(ParsedTimelineMessage {
            event_id,
            sender,
            timestamp,
            body,
            formatted_body,
            message_type,
            image_url,
            custom_emojis,
            encrypted: !matches!(decryption_status, MatrixMessageDecryptionStatus::Plaintext),
            decryption_status,
            verification_status,
        });
    }

    if event_type == event_types::ROOM_ENCRYPTED {
        return Some(ParsedTimelineMessage {
            event_id,
            sender,
            timestamp,
            body: String::from("Unable to decrypt encrypted message"),
            formatted_body: None,
            message_type: None,
            image_url: None,
            custom_emojis: Vec::new(),
            encrypted: true,
            decryption_status: MatrixMessageDecryptionStatus::UnableToDecrypt,
            verification_status: MatrixMessageVerificationStatus::Unknown,
        });
    }

    None
}

fn extract_media_event_url(event: &Value) -> Option<&str> {
    let content = event.get("content")?;

    content
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| {
            content
                .get("file")
                .and_then(|file| file.get("url"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            content
                .get("info")
                .and_then(|info| info.get("thumbnail_url"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            content
                .get("info")
                .and_then(|info| info.get("thumbnail_file"))
                .and_then(|file| file.get("url"))
                .and_then(Value::as_str)
        })
}

fn parse_custom_emojis_from_formatted_body(formatted_body: &str) -> Vec<ParsedCustomEmoji> {
    let mut emojis = Vec::new();
    let mut search_index = 0_usize;

    while let Some(relative_start) = formatted_body[search_index..].find("<img") {
        let tag_start = search_index + relative_start;
        let Some(relative_end) = formatted_body[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_end + 1;
        let tag = &formatted_body[tag_start..tag_end];

        search_index = tag_end;

        if !tag.contains("data-mx-emoticon") {
            continue;
        }

        let Some(shortcode) = extract_html_attribute(tag, "alt") else {
            continue;
        };
        let Some(raw_src) = extract_html_attribute(tag, "src") else {
            continue;
        };

        if shortcode.is_empty() {
            continue;
        }

        emojis.push(ParsedCustomEmoji {
            shortcode,
            url: raw_src,
        });
    }

    emojis
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

#[cfg(test)]
mod tests {
    use serde_json::json;
    use url::Url;

    use crate::messages::{MatrixMessageDecryptionStatus, MatrixMessageVerificationStatus};

    use super::{parse_reaction_event, parse_timeline_message};

    #[test]
    fn parses_plain_text_message() {
        let homeserver = Url::parse("https://matrix.example.org").expect("homeserver");
        let event = json!({
            "type": "m.room.message",
            "event_id": "$abc",
            "sender": "@alice:example.org",
            "origin_server_ts": 123,
            "content": {
                "msgtype": "m.text",
                "body": "hello"
            }
        });

        let parsed = parse_timeline_message(
            &event,
            &homeserver,
            MatrixMessageDecryptionStatus::Plaintext,
            MatrixMessageVerificationStatus::Unknown,
        )
        .expect("message should parse");
        assert_eq!(parsed.body, "hello");
        assert_eq!(parsed.message_type, Some("m.text".to_owned()));
        assert!(parsed.image_url.is_none());
        assert!(!parsed.encrypted);
        assert!(matches!(
            parsed.decryption_status,
            MatrixMessageDecryptionStatus::Plaintext
        ));
    }

    #[test]
    fn parses_image_message_with_mxc_media_url() {
        let homeserver = Url::parse("https://matrix.example.org").expect("homeserver");
        let event = json!({
            "type": "m.room.message",
            "sender": "@alice:example.org",
            "content": {
                "msgtype": "m.image",
                "body": "image",
                "url": "mxc://media.example.org/abcdef"
            }
        });

        let parsed = parse_timeline_message(
            &event,
            &homeserver,
            MatrixMessageDecryptionStatus::Decrypted,
            MatrixMessageVerificationStatus::Verified,
        )
        .expect("message should parse");
        assert_eq!(parsed.body, "image");
        assert_eq!(parsed.message_type, Some("m.image".to_owned()));
        assert_eq!(
            parsed.image_url,
            Some(
                "https://matrix.example.org/_matrix/media/v3/download/media.example.org/abcdef?allow_redirect=true"
                    .to_owned()
            )
        );
        assert!(matches!(
            parsed.verification_status,
            MatrixMessageVerificationStatus::Verified
        ));
    }

    #[test]
    fn parses_image_message_with_file_url() {
        let homeserver = Url::parse("https://matrix.example.org").expect("homeserver");
        let event = json!({
            "type": "m.room.message",
            "sender": "@alice:example.org",
            "content": {
                "msgtype": "m.image",
                "body": "encrypted image",
                "file": {
                    "url": "mxc://media.example.org/encrypted-media"
                }
            }
        });

        let parsed = parse_timeline_message(
            &event,
            &homeserver,
            MatrixMessageDecryptionStatus::Decrypted,
            MatrixMessageVerificationStatus::Verified,
        )
        .expect("message should parse");
        assert_eq!(parsed.message_type, Some("m.image".to_owned()));
        assert_eq!(
            parsed.image_url,
            Some(
                "https://matrix.example.org/_matrix/media/v3/download/media.example.org/encrypted-media?allow_redirect=true"
                    .to_owned()
            )
        );
    }

    #[test]
    fn parses_custom_emoji_from_formatted_body() {
        let homeserver = Url::parse("https://matrix.example.org").expect("homeserver");
        let event = json!({
            "type": "m.room.message",
            "sender": "@alice:example.org",
            "content": {
                "msgtype": "m.text",
                "body": "hello :wave:",
                "formatted_body": "<p>hello <img data-mx-emoticon src=\"mxc://media.example.org/wave\" alt=\":wave:\"></p>"
            }
        });

        let parsed = parse_timeline_message(
            &event,
            &homeserver,
            MatrixMessageDecryptionStatus::Plaintext,
            MatrixMessageVerificationStatus::Unknown,
        )
        .expect("message should parse");

        assert_eq!(parsed.custom_emojis.len(), 1);
        assert_eq!(parsed.custom_emojis[0].shortcode, ":wave:");
        assert_eq!(
            parsed.custom_emojis[0].url,
            "mxc://media.example.org/wave"
        );
    }

    #[test]
    fn parses_reaction_event() {
        let event = json!({
            "type": "m.reaction",
            "event_id": "$reaction",
            "sender": "@alice:example.org",
            "content": {
                "m.relates_to": {
                    "rel_type": "m.annotation",
                    "event_id": "$target",
                    "key": "👍"
                }
            }
        });

        let parsed = parse_reaction_event(&event).expect("reaction should parse");
        assert_eq!(parsed.target_event_id, "$target");
        assert_eq!(parsed.key, "👍");
        assert_eq!(parsed.sender, "@alice:example.org");
    }

    #[test]
    fn maps_unsupported_msgtype_to_placeholder() {
        let homeserver = Url::parse("https://matrix.example.org").expect("homeserver");
        let event = json!({
            "type": "m.room.message",
            "sender": "@alice:example.org",
            "content": {
                "msgtype": "m.audio",
                "body": "audio"
            }
        });

        let parsed = parse_timeline_message(
            &event,
            &homeserver,
            MatrixMessageDecryptionStatus::Decrypted,
            MatrixMessageVerificationStatus::Verified,
        )
        .expect("message should parse");
        assert_eq!(parsed.message_type, Some("m.audio".to_owned()));
        assert_eq!(parsed.body, "Unsupported message type: m.audio");
        assert!(parsed.image_url.is_none());
    }

    #[test]
    fn parses_encrypted_event_fallback() {
        let homeserver = Url::parse("https://matrix.example.org").expect("homeserver");
        let event = json!({
            "type": "m.room.encrypted",
            "sender": "@alice:example.org"
        });

        let parsed = parse_timeline_message(
            &event,
            &homeserver,
            MatrixMessageDecryptionStatus::UnableToDecrypt,
            MatrixMessageVerificationStatus::Unknown,
        )
        .expect("message should parse");
        assert!(parsed.message_type.is_none());
        assert_eq!(parsed.body, "Unable to decrypt encrypted message");
        assert!(parsed.image_url.is_none());
        assert!(parsed.encrypted);
        assert!(matches!(
            parsed.decryption_status,
            MatrixMessageDecryptionStatus::UnableToDecrypt
        ));
    }
}
