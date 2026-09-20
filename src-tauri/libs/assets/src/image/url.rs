use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use log::warn;
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::ruma::events::room::MediaSource;

use super::cache_key::ImageCacheKeyParts;
use super::mime::{
    image_extension_from_mime, image_extension_from_raw_url, mime_type_from_extension,
};
use super::persistence::{
    cached_media_path_for_source_url, persist_normalized_media, register_cached_media_path,
    NormalizedMediaLoad,
};

pub async fn cache_mxc_media_to_local_path(
    client: &matrix_sdk::Client,
    raw_url: &str,
) -> Option<String> {
    if !raw_url.starts_with("mxc://") {
        return None;
    }

    if let Some(cached_path) = cached_media_path_for_source_url(raw_url) {
        return Some(cached_path);
    }

    let mxc_uri = matrix_sdk::ruma::OwnedMxcUri::from(raw_url);
    let request = MediaRequestParameters {
        source: MediaSource::Plain(mxc_uri),
        format: MediaFormat::File,
    };

    let bytes = match client.media().get_media_content(&request, true).await {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!("Failed to fetch MXC image media content: {error}");
            return None;
        }
    };

    let file_stem = mxc_image_cache_key(raw_url, &bytes);
    let extension = image_extension_from_raw_url(raw_url);
    let mime_type = mime_type_from_extension(extension);

    let request = NormalizedMediaLoad::builder()
        .bytes(&bytes)
        .file_stem(file_stem)
        .extension(extension)
        .mime_type(mime_type)
        .build()?;

    let resolved_path = persist_normalized_media(&request)?;
    register_cached_media_path(raw_url, &resolved_path);
    Some(resolved_path)
}

pub async fn resolve_pack_media_url(client: &matrix_sdk::Client, raw_url: &str) -> Option<String> {
    if raw_url.starts_with("mxc://") {
        return cache_mxc_media_to_local_path(client, raw_url).await;
    }

    if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
        if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
            return cache_mxc_media_to_local_path(client, &mxc_url).await;
        }

        warn!(
            "Ignoring non-Matrix HTTP media URL because image fetching is Matrix SDK-only: {}",
            raw_url
        );
        return None;
    }

    None
}

pub fn canonical_pack_source_url(raw_url: &str) -> String {
    if raw_url.starts_with("mxc://") {
        return raw_url.to_owned();
    }

    if let Some(mxc_url) = mxc_from_matrix_media_download_url(raw_url) {
        return mxc_url;
    }

    raw_url.to_owned()
}

pub fn cache_event_image(bytes: &[u8], key_parts: ImageCacheKeyParts) -> Option<String> {
    let extension = image_extension_from_mime(&key_parts.mime_type);
    let file_stem = image_cache_key(&key_parts);

    let request = NormalizedMediaLoad::builder()
        .bytes(bytes)
        .file_stem(file_stem)
        .extension(extension)
        .mime_type(key_parts.mime_type)
        .build()?;

    persist_normalized_media(&request)
}

fn mxc_image_cache_key(raw_url: &str, bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    raw_url.hash(&mut hasher);
    bytes.len().hash(&mut hasher);
    format!("img-{:016x}", hasher.finish())
}

fn image_cache_key(parts: &ImageCacheKeyParts) -> String {
    let mut hasher = DefaultHasher::new();
    parts.event_id.hash(&mut hasher);
    parts.origin_server_ts.hash(&mut hasher);
    parts.room_id.hash(&mut hasher);
    parts.source_key.hash(&mut hasher);
    parts.mime_type.hash(&mut hasher);
    parts.bytes_len.hash(&mut hasher);
    format!("img-{:016x}", hasher.finish())
}

fn mxc_from_matrix_media_download_url(raw_url: &str) -> Option<String> {
    let parsed = url::Url::parse(raw_url).ok()?;
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

#[cfg(test)]
mod tests {
    use super::{canonical_pack_source_url, resolve_pack_media_url};

    #[test]
    fn canonical_pack_source_url_converts_matrix_download_http_url() {
        let canonical = canonical_pack_source_url(
            "https://matrix.example.org/_matrix/media/v3/download/media.example.org/abc123",
        );
        assert_eq!(canonical, "mxc://media.example.org/abc123");
    }

    #[tokio::test]
    async fn resolve_pack_media_url_rejects_non_matrix_http_urls() {
        let homeserver_url = url::Url::parse("https://example.org")
            .expect("homeserver URL should parse for test setup");
        let client = matrix_sdk::Client::new(homeserver_url)
            .await
            .expect("client should construct for URL-only validation");

        let resolved = resolve_pack_media_url(&client, "https://example.org/image.png").await;
        assert!(resolved.is_none());
    }
}
