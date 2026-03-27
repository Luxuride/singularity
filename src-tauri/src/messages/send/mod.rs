use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use std::collections::HashMap;

use super::types::MatrixPickerCustomEmoji;

enum HtmlSegment {
    Text(String),
    LineBreak,
    Emoji { source_url: String, token: String },
}

fn build_formatted_body_from_custom_emoji(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    if !body.contains(':') {
        return None;
    }

    let mut source_by_shortcode = HashMap::<String, String>::new();
    for emoji in picker_custom_emoji {
        let source = emoji.source_url.trim();
        if source.is_empty() || !source.starts_with("mxc://") {
            continue;
        }

        for shortcode in &emoji.shortcodes {
            let shortcode_key = shortcode.trim().trim_matches(':');
            if shortcode_key.is_empty() {
                continue;
            }

            source_by_shortcode
                .entry(shortcode_key.to_owned())
                .or_insert_with(|| source.to_owned());
        }
    }

    if source_by_shortcode.is_empty() {
        return None;
    }

    let mut segments = Vec::<HtmlSegment>::new();
    let mut chars = body.char_indices().peekable();
    let mut replaced_any = false;
    let mut text_start = 0usize;

    while let Some((idx, ch)) = chars.next() {
        if ch != ':' {
            continue;
        }

        if idx > text_start {
            push_text_segments(&mut segments, &body[text_start..idx]);
        }

        let start = idx;
        let mut end = None;
        while let Some((candidate_idx, candidate_ch)) = chars.peek().copied() {
            if candidate_ch == ':' {
                end = Some(candidate_idx);
                break;
            }

            if !(candidate_ch.is_ascii_alphanumeric()
                || candidate_ch == '_'
                || candidate_ch == '+'
                || candidate_ch == '-')
            {
                break;
            }

            let _ = chars.next();
        }

        let Some(end_idx) = end else {
            push_text_segments(&mut segments, &body[start..start + 1]);
            text_start = start + 1;
            continue;
        };

        let shortcode = &body[start + 1..end_idx];
        if shortcode.is_empty() {
            push_text_segments(&mut segments, &body[start..start + 1]);
            text_start = start + 1;
            continue;
        }

        let Some(source_url) = source_by_shortcode.get(shortcode) else {
            push_text_segments(&mut segments, &body[start..=end_idx]);
            let _ = chars.next();
            text_start = end_idx + 1;
            continue;
        };

        let token = format!(":{}:", shortcode);
        segments.push(HtmlSegment::Emoji {
            source_url: source_url.to_owned(),
            token,
        });
        replaced_any = true;
        let _ = chars.next();
        text_start = end_idx + 1;
    }

    if text_start < body.len() {
        push_text_segments(&mut segments, &body[text_start..]);
    }

    if !replaced_any {
        return None;
    }

    let formatted = markup::new! {
        p {
            @for segment in &segments {
                @match segment {
                    HtmlSegment::Text(value) => {
                        @value
                    }
                    HtmlSegment::LineBreak => {
                        br {}
                    }
                    HtmlSegment::Emoji { source_url, token } => {
                        img[data_mx_emoticon = "", src = source_url, alt = token, title = token, height = "32"] {}
                    }
                }
            }
        }
    }
    .to_string();

    Some(formatted)
}

fn push_text_segments(segments: &mut Vec<HtmlSegment>, value: &str) {
    if value.is_empty() {
        return;
    }

    let mut start = 0usize;
    for (idx, ch) in value.char_indices() {
        if ch != '\n' {
            continue;
        }

        if idx > start {
            segments.push(HtmlSegment::Text(value[start..idx].to_owned()));
        }

        segments.push(HtmlSegment::LineBreak);
        start = idx + 1;
    }

    if start < value.len() {
        segments.push(HtmlSegment::Text(value[start..].to_owned()));
    }
}

pub(crate) trait MessageSender {
    async fn send_room_message(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        body: &str,
        picker_custom_emoji: &[MatrixPickerCustomEmoji],
    ) -> Result<String, String>;
}

#[derive(Default, Clone, Copy)]
pub(crate) struct MatrixMessageSender;

impl MessageSender for MatrixMessageSender {
    async fn send_room_message(
        &self,
        client: &matrix_sdk::Client,
        room_id_raw: &str,
        body: &str,
        picker_custom_emoji: &[MatrixPickerCustomEmoji],
    ) -> Result<String, String> {
        let trimmed_body = body.trim();
        if trimmed_body.is_empty() {
            return Err(String::from("Message cannot be empty"));
        }

        let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_raw)
            .map_err(|_| String::from("roomId is invalid"))?;

        let room = client
            .get_room(&room_id)
            .ok_or_else(|| String::from("Room is not available in current session"))?;

        let formatted_body =
            build_formatted_body_from_custom_emoji(trimmed_body, picker_custom_emoji);

        let content = if let Some(formatted_body) = formatted_body.as_deref() {
            RoomMessageEventContent::text_html(trimmed_body, formatted_body)
        } else {
            RoomMessageEventContent::text_plain(trimmed_body)
        };

        let response = room
            .send(content)
            .await
            .map_err(|error| format!("Failed to send room message: {error}"))?;

        Ok(response.event_id.to_string())
    }
}

pub(crate) async fn send_room_message_from_client(
    client: &matrix_sdk::Client,
    room_id_raw: &str,
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Result<String, String> {
    MatrixMessageSender
        .send_room_message(client, room_id_raw, body, picker_custom_emoji)
        .await
}
