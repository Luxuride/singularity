use serde_json::Value;

use crate::messages::{MatrixMessageDecryptionStatus, MatrixMessageVerificationStatus};
use crate::protocol::event_types;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedTimelineMessage {
    pub event_id: Option<String>,
    pub sender: String,
    pub timestamp: Option<u64>,
    pub body: String,
    pub encrypted: bool,
    pub decryption_status: MatrixMessageDecryptionStatus,
    pub verification_status: MatrixMessageVerificationStatus,
}

pub fn parse_timeline_message(
    event: &Value,
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
            body
        } else {
            format!("Unsupported message type: {msgtype}")
        };

        return Some(ParsedTimelineMessage {
            event_id,
            sender,
            timestamp,
            body: text_body,
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
            encrypted: true,
            decryption_status: MatrixMessageDecryptionStatus::UnableToDecrypt,
            verification_status: MatrixMessageVerificationStatus::Unknown,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::messages::{MatrixMessageDecryptionStatus, MatrixMessageVerificationStatus};

    use super::parse_timeline_message;

    #[test]
    fn parses_plain_text_message() {
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
            MatrixMessageDecryptionStatus::Plaintext,
            MatrixMessageVerificationStatus::Unknown,
        )
        .expect("message should parse");
        assert_eq!(parsed.body, "hello");
        assert!(!parsed.encrypted);
        assert!(matches!(
            parsed.decryption_status,
            MatrixMessageDecryptionStatus::Plaintext
        ));
    }

    #[test]
    fn maps_unsupported_msgtype_to_placeholder() {
        let event = json!({
            "type": "m.room.message",
            "sender": "@alice:example.org",
            "content": {
                "msgtype": "m.image",
                "body": "image"
            }
        });

        let parsed = parse_timeline_message(
            &event,
            MatrixMessageDecryptionStatus::Decrypted,
            MatrixMessageVerificationStatus::Verified,
        )
        .expect("message should parse");
        assert_eq!(parsed.body, "Unsupported message type: m.image");
        assert!(matches!(
            parsed.verification_status,
            MatrixMessageVerificationStatus::Verified
        ));
    }

    #[test]
    fn parses_encrypted_event_fallback() {
        let event = json!({
            "type": "m.room.encrypted",
            "sender": "@alice:example.org"
        });

        let parsed = parse_timeline_message(
            &event,
            MatrixMessageDecryptionStatus::UnableToDecrypt,
            MatrixMessageVerificationStatus::Unknown,
        )
        .expect("message should parse");
        assert_eq!(parsed.body, "Unable to decrypt encrypted message");
        assert!(parsed.encrypted);
        assert!(matches!(
            parsed.decryption_status,
            MatrixMessageDecryptionStatus::UnableToDecrypt
        ));
    }
}
