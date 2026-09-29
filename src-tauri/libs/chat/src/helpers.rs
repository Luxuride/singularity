use assets::media_url_is_available;
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::api::Direction;
use matrix_sdk::ruma::uint;
use types::chat::{MatrixChatMessage, MatrixMessageDecryptionStatus};

/// The room-unavailable error the Matrix SDK produces for a room that is not
/// (yet) in the local session. The SDK also emits a "…current session yet"
/// variant, which is a prefix of this, so a `contains` check covers both.
pub const ROOM_NOT_AVAILABLE: &str = "Room is not available in current session";

/// Whether a cached timeline can no longer be served as-is.
///
/// Two kinds of staleness:
///
/// * **Missing media.** Media is disk-backed, so a cached `asset://` URL is
///   stale once its file is evicted. Videos carry their poster in
///   `thumbnail_url` and leave `image_url` unset, and custom emoji `src`
///   values are disk-backed too, so all three are checked.
/// * **Undecryptable placeholders.** A UTD message has no media to check, and
///   once its keys arrive from another device the cache would still say
///   "cannot decrypt".
pub fn has_stale_cached_media_urls(messages: &[MatrixChatMessage]) -> bool {
    messages.iter().any(|message| {
        let media_urls = [
            message.image_url.as_deref(),
            message.thumbnail_url.as_deref(),
        ]
        .into_iter()
        .flatten()
        .chain(message.custom_emojis.iter().map(|emoji| emoji.url.as_str()));

        if media_urls
            .into_iter()
            .any(|url| !media_url_is_available(url))
        {
            return true;
        }

        message.decryption_status == MatrixMessageDecryptionStatus::UnableToDecrypt
    })
}

pub fn is_room_unavailable_error(error: &str) -> bool {
    error.contains(ROOM_NOT_AVAILABLE)
}

/// The lower and upper bounds the frontend and the streamer agree on for a
/// single pagination request. These lived in two places and had already
/// diverged: the streamer clamped the floor as well, this helper only the
/// ceiling, so a caller asking for zero messages got an empty page.
pub const MIN_PAGE_SIZE: u32 = 1;
pub const MAX_PAGE_SIZE: u32 = 100;

/// Build `MessagesOptions` for a backward pagination request, defaulting to a
/// 50-message limit.
pub fn build_messages_options(from: Option<String>, limit: Option<u32>) -> MessagesOptions {
    let mut options = MessagesOptions::new(Direction::Backward);
    options.from = from;
    options.limit = uint!(50);
    if let Some(limit) = limit {
        options.limit = limit.clamp(MIN_PAGE_SIZE, MAX_PAGE_SIZE).into();
    }
    options
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::has_stale_cached_media_urls;
    use types::chat::{
        MatrixChatMessage, MatrixCustomEmoji, MatrixMessageDecryptionStatus,
        MatrixMessageVerificationStatus,
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
    fn an_evicted_video_poster_invalidates_the_cache() {
        // Videos carry the poster in `thumbnail_url` and leave `image_url` unset.
        let mut message = message_with_image(None);
        message.thumbnail_url = Some(String::from(
            "asset://localhost/%2Ftmp%2Fsingularity-test%2Fmissing%2Fposter.png",
        ));

        assert!(has_stale_cached_media_urls(&[message]));
    }

    #[test]
    fn an_undecryptable_placeholder_invalidates_the_cache() {
        // Nothing about a UTD message is media, so it never trips the file check.
        let mut message = message_with_image(None);
        message.decryption_status = MatrixMessageDecryptionStatus::UnableToDecrypt;

        assert!(has_stale_cached_media_urls(&[message]));
    }

    #[test]
    fn an_evicted_custom_emoji_invalidates_the_cache() {
        let mut message = message_with_image(None);
        message.custom_emojis = vec![MatrixCustomEmoji {
            shortcode: String::from("wave"),
            url: String::from("asset://localhost/%2Ftmp%2Fsingularity-test%2Fmissing%2Fwave.png"),
        }];

        assert!(has_stale_cached_media_urls(&[message]));
    }

    #[test]
    fn a_fully_resolved_message_is_not_stale() {
        let dir = std::env::temp_dir().join("singularity-test-media-complete");
        fs::create_dir_all(&dir).expect("create temp media dir");
        let image = dir.join("img.png");
        let emoji = dir.join("wave.png");
        fs::write(&image, [1, 2, 3]).expect("write temp image");
        fs::write(&emoji, [1, 2, 3]).expect("write temp emoji");
        let encode = |path: &std::path::Path| {
            format!(
                "asset://localhost/{}",
                path.to_string_lossy().replace("/", "%2F")
            )
        };

        let mut message = message_with_image(Some(&encode(&image)));
        message.thumbnail_url = Some(encode(&emoji));
        message.custom_emojis = vec![MatrixCustomEmoji {
            shortcode: String::from("wave"),
            url: encode(&emoji),
        }];

        assert!(!has_stale_cached_media_urls(&[message]));

        fs::remove_dir_all(&dir).expect("clean up temp media dir");
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
