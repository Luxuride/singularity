use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::ruma::events::room::MediaSource;
use rand::distributions::{Alphanumeric, DistString};

const STREAM_TTL: Duration = Duration::from_secs(15 * 60);

struct VideoStreamEntry {
    media_source: MediaSource,
    mime_type: String,
    bytes: Option<Vec<u8>>,
    created_at: Instant,
}

pub enum VideoStreamReadOutcome {
    NotFound,
    NotSatisfiable { total_len: usize },
    Segment(VideoStreamSegment),
}

pub struct VideoStreamSegment {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub total_len: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Default)]
pub struct VideoStreamState {
    entries: Mutex<HashMap<String, VideoStreamEntry>>,
}

impl VideoStreamState {
    pub fn create_stream_url(
        &self,
        media_source: MediaSource,
        mime_type: String,
    ) -> Result<String, String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| String::from("Failed to lock video stream state"))?;

        purge_expired(&mut entries);

        let mut token = random_stream_token();
        while entries.contains_key(token.as_str()) {
            token = random_stream_token();
        }

        entries.insert(
            token.clone(),
            VideoStreamEntry {
                media_source,
                mime_type,
                bytes: None,
                created_at: Instant::now(),
            },
        );

        Ok(format!("matrix-media://stream/{token}"))
    }

    pub async fn read_stream_segment(
        &self,
        client: &matrix_sdk::Client,
        token: &str,
        start: usize,
        requested_end: Option<usize>,
    ) -> Result<VideoStreamReadOutcome, String> {
        let needs_load = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| String::from("Failed to lock video stream state"))?;

            purge_expired(&mut entries);

            let Some(entry) = entries.get(token) else {
                return Ok(VideoStreamReadOutcome::NotFound);
            };

            entry.bytes.is_none()
        };

        if needs_load {
            let media_source = {
                let mut entries = self
                    .entries
                    .lock()
                    .map_err(|_| String::from("Failed to lock video stream state"))?;

                purge_expired(&mut entries);

                let Some(entry) = entries.get(token) else {
                    return Ok(VideoStreamReadOutcome::NotFound);
                };

                entry.media_source.clone()
            };

            let request = MediaRequestParameters {
                source: media_source,
                format: MediaFormat::File,
            };

            let bytes = client
                .media()
                .get_media_content(&request, true)
                .await
                .map_err(|error| format!("Failed to fetch video media content: {error}"))?;

            let mut entries = self
                .entries
                .lock()
                .map_err(|_| String::from("Failed to lock video stream state"))?;

            purge_expired(&mut entries);

            let Some(entry) = entries.get_mut(token) else {
                return Ok(VideoStreamReadOutcome::NotFound);
            };

            entry.bytes = Some(bytes);
        }

        let mut entries = self
            .entries
            .lock()
            .map_err(|_| String::from("Failed to lock video stream state"))?;

        purge_expired(&mut entries);

        let Some(entry) = entries.get(token) else {
            return Ok(VideoStreamReadOutcome::NotFound);
        };

        let Some(bytes) = entry.bytes.as_ref() else {
            return Ok(VideoStreamReadOutcome::NotFound);
        };

        let total_len = bytes.len();
        if total_len == 0 || start >= total_len {
            return Ok(VideoStreamReadOutcome::NotSatisfiable { total_len });
        }

        let end = requested_end
            .unwrap_or_else(|| total_len.saturating_sub(1))
            .min(total_len.saturating_sub(1));
        if end < start {
            return Ok(VideoStreamReadOutcome::NotSatisfiable { total_len });
        }

        let segment = bytes[start..=end].to_vec();
        Ok(VideoStreamReadOutcome::Segment(VideoStreamSegment {
            bytes: segment,
            mime_type: entry.mime_type.clone(),
            total_len,
            start,
            end,
        }))
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| String::from("Failed to lock video stream state"))?;

        entries.clear();
        Ok(())
    }
}

fn purge_expired(entries: &mut HashMap<String, VideoStreamEntry>) {
    let now = Instant::now();
    entries.retain(|_, entry| now.duration_since(entry.created_at) <= STREAM_TTL);
}

fn random_stream_token() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 32)
}
