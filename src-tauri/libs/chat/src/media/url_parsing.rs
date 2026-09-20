use matrix_sdk::ruma::events::room::{EncryptedFile, MediaSource};
use serde_json::Value;

pub(super) fn image_source_key(event: &Value) -> Option<&str> {
    event
        .get("content")
        .and_then(|content| content.get("url"))
        .and_then(Value::as_str)
        .or_else(|| {
            event
                .get("content")
                .and_then(|content| content.get("file"))
                .and_then(|file| file.get("url"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            event
                .get("content")
                .and_then(|content| content.get("info"))
                .and_then(|info| info.get("thumbnail_url"))
                .and_then(Value::as_str)
        })
}

/// Build a `MediaSource` from the media fields of an event's content.
///
/// The content object is not itself a `MediaSource`; it carries the media in
/// either a plain `url` field or an encrypted `file` object. Deserializing the
/// whole content into `MediaSource` would always fail, so we construct the
/// source explicitly from those fields.
pub(super) fn image_media_source_from_event(event: &Value) -> Option<MediaSource> {
    let content = event.get("content")?;

    if let Some(url) = content.get("url").and_then(Value::as_str) {
        let mxc_uri = matrix_sdk::ruma::OwnedMxcUri::from(url);
        return Some(MediaSource::Plain(mxc_uri));
    }

    if let Some(file) = content.get("file") {
        if let Ok(encrypted_file) = serde_json::from_value::<EncryptedFile>(file.clone()) {
            return Some(MediaSource::Encrypted(Box::new(encrypted_file)));
        }
    }

    None
}

pub(super) fn image_mime_type_from_event(event: &Value) -> Option<String> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("mimetype"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

/// Build a `MediaSource` for the thumbnail of a media event's content.
///
/// Matrix media events may carry a thumbnail in `content.info.thumbnail_url`
/// (plain) or `content.info.thumbnail_file` (encrypted). Returns `None` when
/// the event has no thumbnail.
pub(super) fn image_thumbnail_source_from_event(event: &Value) -> Option<MediaSource> {
    let content = event.get("content")?;
    let info = content.get("info")?;

    if let Some(url) = info.get("thumbnail_url").and_then(Value::as_str) {
        let mxc_uri = matrix_sdk::ruma::OwnedMxcUri::from(url);
        return Some(MediaSource::Plain(mxc_uri));
    }

    if let Some(file) = info.get("thumbnail_file") {
        if let Ok(encrypted_file) = serde_json::from_value::<EncryptedFile>(file.clone()) {
            return Some(MediaSource::Encrypted(Box::new(encrypted_file)));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use matrix_sdk::ruma::events::room::MediaSource;
    use serde_json::json;

    use super::image_media_source_from_event;

    #[test]
    fn builds_plain_source_from_url_field() {
        let event = json!({
            "content": {
                "body": "image",
                "msgtype": "m.image",
                "url": "mxc://example.org/abc123",
                "info": { "mimetype": "image/png" },
            }
        });

        let source = image_media_source_from_event(&event).expect("plain source");
        assert!(matches!(source, MediaSource::Plain(_)));
    }

    #[test]
    fn builds_encrypted_source_from_file_field() {
        let event = json!({
            "content": {
                "body": "image",
                "msgtype": "m.image",
                "file": {
                    "url": "mxc://example.org/abc123",
                    "key": {
                        "kty": "oct",
                        "key_ops": ["encrypt", "decrypt"],
                        "alg": "A256CTR",
                        "k": "b50ACIv6LMn9AfMCFD1POJI_UAFWIclxAN1kWrEO2X8",
                        "ext": true,
                    },
                    "iv": "AK1wyzigZtQAAAABAAAAKK",
                    "hashes": {
                        "sha256": "SBbJ3hINT2LgwXK8ev82enjnhubUy5UuKGDF3SezAhs",
                    },
                    "v": "v2",
                },
            }
        });

        let source = image_media_source_from_event(&event).expect("encrypted source");
        assert!(matches!(source, MediaSource::Encrypted(_)));
    }

    #[test]
    fn returns_none_without_media_fields() {
        let event = json!({
            "content": {
                "body": "hello",
                "msgtype": "m.text",
            }
        });

        assert!(image_media_source_from_event(&event).is_none());
    }
}
