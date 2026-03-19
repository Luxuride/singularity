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
    pub message_type: Option<String>,
    pub image_url: Option<String>,
    pub video_thumbnail_url: Option<String>,
    pub video_mime_type: Option<String>,
    pub video_size_bytes: Option<u64>,
    pub video_duration_ms: Option<u64>,
    pub encrypted: bool,
    pub decryption_status: MatrixMessageDecryptionStatus,
    pub verification_status: MatrixMessageVerificationStatus,
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
        let msgtype = event
            .get("content")
            .and_then(|content| content.get("msgtype"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let is_video_message = is_video_message_event(msgtype, event);
        let message_type = if msgtype.is_empty() {
            None
        } else if is_video_message {
            Some(event_types::message_types::VIDEO.to_owned())
        } else {
            Some(msgtype.to_owned())
        };
        let body = event
            .get("content")
            .and_then(|content| content.get("body"))
            .and_then(Value::as_str)
            .unwrap_or("Unsupported message")
            .to_owned();

        let text_body = if msgtype == event_types::message_types::TEXT
            || msgtype == event_types::message_types::NOTICE
            || msgtype == event_types::message_types::EMOTE
        {
            Some((body, None, None, None, None, None))
        } else if msgtype == event_types::message_types::IMAGE {
            let image_url = extract_image_event_url(event)
                .and_then(|raw| matrix_media_url_from_event_url(homeserver_url, raw));

            Some((body, image_url, None, None, None, None))
        } else if is_video_message {
            let video_thumbnail_url = extract_video_thumbnail_event_url(event)
                .and_then(|raw| matrix_media_url_from_event_url(homeserver_url, raw));
            let video_mime_type = extract_video_mime_type(event);
            let video_size_bytes = extract_video_size(event);
            let video_duration_ms = extract_video_duration(event);

            Some((
                body,
                None,
                video_thumbnail_url,
                video_mime_type,
                video_size_bytes,
                video_duration_ms,
            ))
        } else {
            None
        };

        let (
            body,
            image_url,
            video_thumbnail_url,
            video_mime_type,
            video_size_bytes,
            video_duration_ms,
        ) = match text_body {
            Some(values) => values,
            None => (format!("Unsupported message type: {msgtype}"), None, None, None, None, None),
        };

        return Some(ParsedTimelineMessage {
            event_id,
            sender,
            timestamp,
            body,
            message_type,
            image_url,
            video_thumbnail_url,
            video_mime_type,
            video_size_bytes,
            video_duration_ms,
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
            message_type: None,
            image_url: None,
            video_thumbnail_url: None,
            video_mime_type: None,
            video_size_bytes: None,
            video_duration_ms: None,
            encrypted: true,
            decryption_status: MatrixMessageDecryptionStatus::UnableToDecrypt,
            verification_status: MatrixMessageVerificationStatus::Unknown,
        });
    }

    None
}

fn is_video_message_event(msgtype: &str, event: &Value) -> bool {
    if msgtype == event_types::message_types::VIDEO
        || msgtype == event_types::message_types::VIDEO_UNSTABLE
    {
        return true;
    }

    if msgtype != event_types::message_types::FILE && msgtype != event_types::message_types::IMAGE {
        return false;
    }

    extract_video_mime_type(event)
        .map(|mimetype| mimetype.to_ascii_lowercase().starts_with("video/"))
        .unwrap_or(false)
}

fn extract_image_event_url(event: &Value) -> Option<&str> {
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

fn extract_video_thumbnail_event_url(event: &Value) -> Option<&str> {
    let content = event.get("content")?;

    content
        .get("info")
        .and_then(|info| info.get("thumbnail_url"))
        .and_then(Value::as_str)
        .or_else(|| {
            content
                .get("info")
                .and_then(|info| info.get("thumbnail_file"))
                .and_then(|file| file.get("url"))
                .and_then(Value::as_str)
        })
}

fn extract_video_mime_type(event: &Value) -> Option<String> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("mimetype"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn extract_video_size(event: &Value) -> Option<u64> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("size"))
        .and_then(Value::as_u64)
}

fn extract_video_duration(event: &Value) -> Option<u64> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("duration"))
        .and_then(Value::as_u64)
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

    use super::parse_timeline_message;

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
        assert!(parsed.video_thumbnail_url.is_none());
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
        assert!(parsed.video_thumbnail_url.is_none());
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
        assert!(parsed.video_thumbnail_url.is_none());
    }

    #[test]
    fn parses_video_message_with_thumbnail_metadata() {
        let homeserver = Url::parse("https://matrix.example.org").expect("homeserver");
        let event = json!({
            "type": "m.room.message",
            "sender": "@alice:example.org",
            "content": {
                "msgtype": "m.video",
                "body": "video",
                "url": "mxc://media.example.org/video",
                "info": {
                    "mimetype": "video/mp4",
                    "size": 1024,
                    "duration": 12000,
                    "thumbnail_url": "mxc://media.example.org/thumb"
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

        assert_eq!(parsed.message_type, Some("m.video".to_owned()));
        assert_eq!(
            parsed.video_thumbnail_url,
            Some(
                "https://matrix.example.org/_matrix/media/v3/download/media.example.org/thumb?allow_redirect=true"
                    .to_owned()
            )
        );
        assert_eq!(parsed.video_mime_type, Some("video/mp4".to_owned()));
        assert_eq!(parsed.video_size_bytes, Some(1024));
        assert_eq!(parsed.video_duration_ms, Some(12000));
    }

    #[test]
    fn parses_video_message_with_encrypted_thumbnail_file() {
        let homeserver = Url::parse("https://matrix.example.org").expect("homeserver");
        let event = json!({
            "type": "m.room.message",
            "sender": "@alice:example.org",
            "content": {
                "msgtype": "m.video",
                "body": "encrypted video",
                "file": {
                    "url": "mxc://media.example.org/video-encrypted"
                },
                "info": {
                    "thumbnail_file": {
                        "url": "mxc://media.example.org/thumb-encrypted"
                    }
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

        assert_eq!(parsed.message_type, Some("m.video".to_owned()));
        assert_eq!(
            parsed.video_thumbnail_url,
            Some(
                "https://matrix.example.org/_matrix/media/v3/download/media.example.org/thumb-encrypted?allow_redirect=true"
                    .to_owned()
            )
        );
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
        assert!(parsed.video_thumbnail_url.is_none());
        assert!(parsed.encrypted);
        assert!(matches!(
            parsed.decryption_status,
            MatrixMessageDecryptionStatus::UnableToDecrypt
        ));
    }
}
