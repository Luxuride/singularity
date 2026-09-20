/// Cache key parts used to derive a stable, content-addressed cache file name
/// for a video event. The parts are hashed together to produce the file stem.

#[derive(Clone, Debug)]
pub struct VideoCacheKeyParts {
    pub source_key: Option<String>,
    pub mime_type: String,
    pub bytes_len: usize,
}

impl VideoCacheKeyParts {
    pub fn builder() -> VideoCacheKeyPartsBuilder {
        VideoCacheKeyPartsBuilder::default()
    }
}

#[derive(Default)]
pub struct VideoCacheKeyPartsBuilder {
    source_key: Option<String>,
    mime_type: Option<String>,
    bytes_len: Option<usize>,
}

impl VideoCacheKeyPartsBuilder {
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

    pub fn build(self) -> Option<VideoCacheKeyParts> {
        Some(VideoCacheKeyParts {
            source_key: self.source_key,
            mime_type: self.mime_type?,
            bytes_len: self.bytes_len?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::VideoCacheKeyParts;

    #[test]
    fn video_cache_key_builder_requires_fields() {
        let missing = VideoCacheKeyParts::builder().mime_type("video/mp4").build();
        assert!(missing.is_none());

        let complete = VideoCacheKeyParts::builder()
            .source_key(Some("mxc://server/media"))
            .mime_type("video/mp4")
            .bytes_len(10)
            .build();
        assert!(complete.is_some());
    }
}
