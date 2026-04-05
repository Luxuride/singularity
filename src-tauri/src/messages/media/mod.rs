use log::warn;
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use serde_json::Value;

use crate::assets::image::{self, ImageCacheKeyParts};

mod url_parsing;
use url_parsing::{
    image_media_source_from_event, image_mime_type_from_event, image_source_key,
    mxc_from_matrix_media_download_url,
};

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

            warn!(
                "Ignoring non-Matrix HTTP media URL because image fetching is Matrix SDK-only: {}",
                raw_url
            );
            return None;
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
