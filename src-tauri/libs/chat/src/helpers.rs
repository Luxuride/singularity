use assets::media_url_is_available;
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::uint;
use types::chat::MatrixChatMessage;

/// Whether any cached message references media that is no longer available on
/// disk. Media is disk-backed, so a cached URL is stale only when its file is
/// missing; in that case the caller re-fetches from the server.
pub fn has_stale_cached_media_urls(messages: &[MatrixChatMessage]) -> bool {
    messages.iter().any(|message| {
        message
            .image_url
            .as_deref()
            .is_some_and(|url| !media_url_is_available(url))
    })
}

pub fn is_room_unavailable_error(error: &str) -> bool {
    error.contains("Room is not available in current session")
}

/// Build `MessagesOptions` for a backward pagination request, defaulting to a
/// 50-message limit (capped at 100 when an explicit limit is provided).
pub fn build_messages_options(from: Option<String>, limit: Option<u32>) -> MessagesOptions {
    let mut options = MessagesOptions::new(Direction::Backward);
    options.from = from;
    options.limit = uint!(50);
    if let Some(limit) = limit {
        options.limit = limit.min(100).into();
    }
    options
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::has_stale_cached_media_urls;
    use types::chat::{
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
            thumbnail_url: None,
            custom_emojis: Vec::new(),
            reactions: Vec::new(),
            encrypted: false,
            decryption_status: MatrixMessageDecryptionStatus::Plaintext,
            verification_status: MatrixMessageVerificationStatus::Unknown,
        }
    }

    #[test]
    fn detects_stale_missing_media_file() {
        let messages = vec![message_with_image(Some(
            "asset://localhost/%2Ftmp%2Fsingularity-test%2Fmissing%2Fimg-123.png",
        ))];

        assert!(has_stale_cached_media_urls(&messages));
    }

    #[test]
    fn ignores_available_media_urls() {
        let dir = std::env::temp_dir().join("singularity-test-media");
        fs::create_dir_all(&dir).expect("create temp media dir");
        let file = dir.join("img-123.png");
        fs::write(&file, [1, 2, 3]).expect("write temp media file");

        // asset:// URLs percent-encode the absolute path.
        let encoded = file.to_string_lossy().replace("/", "%2F");
        let file_url = format!("asset://localhost/{}", encoded);

        let messages = vec![
            message_with_image(None),
            message_with_image(Some(file_url.as_str())),
        ];

        assert!(!has_stale_cached_media_urls(&messages));

        fs::remove_file(&file).expect("clean up temp media file");
        fs::remove_dir(&dir).expect("clean up temp media dir");
    }
}
