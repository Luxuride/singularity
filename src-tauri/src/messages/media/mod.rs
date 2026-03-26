use log::warn;
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::ruma::events::room::MediaSource;
use serde_json::Value;
use url::Url;

use crate::assets::image::{self, ImageCacheKeyParts};

pub(crate) trait MediaResolver {
    async fn resolve_pack_media_url(
        &self,
        client: &matrix_sdk::Client,
        raw_url: &str,
    ) -> Option<String>;
    fn canonical_pack_source_url(&self, raw_url: &str) -> String;
    async fn resolve_image_cache_path(
        &self,
        client: &matrix_sdk::Client,
        event: &Value,
    ) -> Option<String>;
    async fn cache_mxc_media_to_local_path(
        &self,
        client: &matrix_sdk::Client,
        raw_url: &str,
    ) -> Option<String>;
}

#[derive(Default, Clone, Copy)]
pub(crate) struct DefaultMediaResolver;

impl MediaResolver for DefaultMediaResolver {
    async fn resolve_pack_media_url(
        &self,
        client: &matrix_sdk::Client,
        raw_url: &str,
    ) -> Option<String> {
        if raw_url.starts_with("mxc://") {
            return image::cache_mxc_media_to_local_path(client, raw_url).await;
        }

        if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
            if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
                if let Some(local) = image::cache_mxc_media_to_local_path(client, &mxc_url).await {
                    return Some(local);
                }

                return None;
            }

            return image::cache_http_media_to_local_path(raw_url).await;
        }

        None
    }

    fn canonical_pack_source_url(&self, raw_url: &str) -> String {
        if raw_url.starts_with("mxc://") {
            return raw_url.to_owned();
        }

        if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
            return mxc_url;
        }

        raw_url.to_owned()
    }

    async fn resolve_image_cache_path(
        &self,
        client: &matrix_sdk::Client,
        event: &Value,
    ) -> Option<String> {
        let media_source = image_media_source_from_event(event)?;
        let mime_type = image_mime_type_from_event(event)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| String::from("application/octet-stream"));

        let request = MediaRequestParameters {
            source: media_source,
            format: MediaFormat::File,
        };

        let bytes = match client.media().get_media_content(&request, true).await {
            Ok(bytes) => bytes,
            Err(error) => {
                warn!("Failed to fetch image media content: {error}");
                return None;
            }
        };

        let cache_key_parts = ImageCacheKeyParts::builder()
            .event_id(event.get("event_id").and_then(Value::as_str))
            .origin_server_ts(event.get("origin_server_ts").and_then(Value::as_u64))
            .room_id(event.get("room_id").and_then(Value::as_str))
            .source_key(image_source_key(event))
            .mime_type(mime_type)
            .bytes_len(bytes.len())
            .build()?;

        image::cache_event_image(&bytes, cache_key_parts)
    }

    async fn cache_mxc_media_to_local_path(
        &self,
        client: &matrix_sdk::Client,
        raw_url: &str,
    ) -> Option<String> {
        image::cache_mxc_media_to_local_path(client, raw_url).await
    }
}

pub(crate) async fn cache_mxc_media_to_local_path(
    client: &matrix_sdk::Client,
    raw_url: &str,
) -> Option<String> {
    DefaultMediaResolver
        .cache_mxc_media_to_local_path(client, raw_url)
        .await
}

fn mxc_from_matrix_media_download_url(raw_url: &str) -> Option<String> {
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

fn image_source_key(event: &Value) -> Option<&str> {
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

fn image_media_source_from_event(event: &Value) -> Option<MediaSource> {
    let content = event.get("content")?;
    serde_json::from_value(content.clone()).ok()
}

fn image_mime_type_from_event(event: &Value) -> Option<String> {
    event
        .get("content")
        .and_then(|content| content.get("info"))
        .and_then(|info| info.get("mimetype"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}
