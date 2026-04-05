use matrix_sdk::ruma::events::room::MediaSource;
use serde_json::Value;
use url::Url;

pub(super) fn mxc_from_matrix_media_download_url(raw_url: &str) -> Option<String> {
    let parsed = Url::parse(raw_url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }

    let segments: Vec<_> = parsed.path_segments()?.collect();
    let download_index = segments.windows(4).position(|window| {
        window.first() == Some(&"_matrix")
            && window.get(1) == Some(&"media")
            && window.get(3) == Some(&"download")
    })?;

    let server_name = segments.get(download_index + 4)?;
    let media_id = segments.get(download_index + 5)?;

    if server_name.is_empty() || media_id.is_empty() {
        return None;
    }

    Some(format!("mxc://{server_name}/{media_id}"))
}

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

pub(super) fn image_media_source_from_event(event: &Value) -> Option<MediaSource> {
    let content = event.get("content")?;
    serde_json::from_value(content.clone()).ok()
}

pub(super) fn image_mime_type_from_event(event: &Value) -> Option<String> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("mimetype"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}
