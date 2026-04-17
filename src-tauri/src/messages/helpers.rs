use super::types::MatrixChatMessage;

pub(super) fn has_stale_in_memory_media_urls(messages: &[MatrixChatMessage]) -> bool {
    messages.iter().any(|message| {
        message
            .image_url
            .as_deref()
            .is_some_and(|url| url.starts_with("matrix-media://"))
    })
}

pub(super) fn is_room_unavailable_error(error: &str) -> bool {
    error.contains("Room is not available in current session")
}

#[cfg(test)]
mod tests {
    use super::has_stale_in_memory_media_urls;
    use crate::messages::types::{
        MatrixChatMessage, MatrixMessageDecryptionStatus, MatrixMessageVerificationStatus,
    };

    fn message_with_image(image_url: Option<&str>) -> MatrixChatMessage {
        MatrixChatMessage {
            event_id: Some(String::from("$event")),
            in_reply_to_event_id: None,
            sender: String::from("@alice:example.org"),
            timestamp: Some(1),
            body: String::from("body"),
            formatted_body: None,
            message_type: Some(String::from("m.image")),
            image_url: image_url.map(ToOwned::to_owned),
            custom_emojis: Vec::new(),
            reactions: Vec::new(),
            encrypted: false,
            decryption_status: MatrixMessageDecryptionStatus::Plaintext,
            verification_status: MatrixMessageVerificationStatus::Unknown,
        }
    }

    #[test]
    fn detects_stale_matrix_media_url() {
        let messages = vec![message_with_image(Some(
            "matrix-media://localhost/img-123.png",
        ))];

        assert!(has_stale_in_memory_media_urls(&messages));
    }

    #[test]
    fn ignores_non_stale_media_urls() {
        let messages = vec![
            message_with_image(None),
            message_with_image(Some("asset://localhost/home/user/.cache/eu.luxuride.singularity/media-cache/img-123.png")),
            message_with_image(Some("https://example.org/media.png")),
        ];

        assert!(!has_stale_in_memory_media_urls(&messages));
    }
}
