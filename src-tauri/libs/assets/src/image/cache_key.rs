/// Cache key parts used to derive a stable, content-addressed cache file name
/// for a media event. The parts are hashed together to produce the file stem.

#[derive(Clone, Debug)]
pub struct ImageCacheKeyParts {
    pub event_id: Option<String>,
    pub origin_server_ts: Option<u64>,
    pub room_id: Option<String>,
    pub source_key: Option<String>,
    pub mime_type: String,
    pub bytes_len: usize,
}

impl ImageCacheKeyParts {
    pub fn builder() -> ImageCacheKeyPartsBuilder {
        ImageCacheKeyPartsBuilder::default()
    }
}

#[derive(Default)]
pub struct ImageCacheKeyPartsBuilder {
    event_id: Option<String>,
    origin_server_ts: Option<u64>,
    room_id: Option<String>,
    source_key: Option<String>,
    mime_type: Option<String>,
    bytes_len: Option<usize>,
}

impl ImageCacheKeyPartsBuilder {
    pub fn event_id<T>(mut self, event_id: Option<T>) -> Self
    where
        T: Into<String>,
    {
        self.event_id = event_id.map(Into::into);
        self
    }

    pub fn origin_server_ts(mut self, origin_server_ts: Option<u64>) -> Self {
        self.origin_server_ts = origin_server_ts;
        self
    }

    pub fn room_id<T>(mut self, room_id: Option<T>) -> Self
    where
        T: Into<String>,
    {
        self.room_id = room_id.map(Into::into);
        self
    }

    pub fn source_key<T>(mut self, source_key: Option<T>) -> Self
    where
        T: Into<String>,
    {
        self.source_key = source_key.map(Into::into);
        self
    }

    pub fn mime_type<T>(mut self, mime_type: T) -> Self
    where
        T: Into<String>,
    {
        self.mime_type = Some(mime_type.into());
        self
    }

    pub fn bytes_len(mut self, bytes_len: usize) -> Self {
        self.bytes_len = Some(bytes_len);
        self
    }

    pub fn build(self) -> Option<ImageCacheKeyParts> {
        Some(ImageCacheKeyParts {
            event_id: self.event_id,
            origin_server_ts: self.origin_server_ts,
            room_id: self.room_id,
            source_key: self.source_key,
            mime_type: self.mime_type?,
            bytes_len: self.bytes_len?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ImageCacheKeyParts;

    #[test]
    fn image_cache_key_builder_requires_fields() {
        let missing = ImageCacheKeyParts::builder().mime_type("image/png").build();
        assert!(missing.is_none());

        let complete = ImageCacheKeyParts::builder()
            .event_id(Some("$abc"))
            .origin_server_ts(Some(123))
            .room_id(Some("!room:server"))
            .source_key(Some("mxc://server/media"))
            .mime_type("image/png")
            .bytes_len(10)
            .build();
        assert!(complete.is_some());
    }
}
