use std::collections::HashMap;

use crate::messages::types::MatrixPickerCustomEmoji;

enum HtmlSegment {
    Text(String),
    LineBreak,
    Emoji { source_url: String, token: String },
}

pub(super) fn build_formatted_body_from_custom_emoji(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    build_formatted_body_with_url_selector(body, picker_custom_emoji, |emoji| {
        let source = emoji.source_url.trim();
        if source.is_empty() || !source.starts_with("mxc://") {
            return None;
        }

        Some(source.to_owned())
    })
}

pub(super) fn build_display_formatted_body_from_custom_emoji(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
) -> Option<String> {
    build_formatted_body_with_url_selector(body, picker_custom_emoji, |emoji| {
        let image_url = emoji.url.trim();
        if image_url.is_empty() {
            return None;
        }

        Some(image_url.to_owned())
    })
}

fn build_formatted_body_with_url_selector<F>(
    body: &str,
    picker_custom_emoji: &[MatrixPickerCustomEmoji],
    mut url_selector: F,
) -> Option<String>
where
    F: FnMut(&MatrixPickerCustomEmoji) -> Option<String>,
{
    if !body.contains(':') {
        return None;
    }

    let mut source_by_shortcode = HashMap::<String, String>::new();
    for emoji in picker_custom_emoji {
        let Some(source) = url_selector(emoji) else {
            continue;
        };

        for shortcode in &emoji.shortcodes {
            let shortcode_key = shortcode.trim().trim_matches(':').to_lowercase();
            if shortcode_key.is_empty() {
                continue;
            }

            source_by_shortcode
                .entry(shortcode_key)
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

        let shortcode_key = shortcode.to_lowercase();

        let Some(source_url) = source_by_shortcode.get(&shortcode_key) else {
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

    // Single emoji messages get double the height (64 vs 32)
    let emoji_height = if segments.len() == 1 && matches!(segments[0], HtmlSegment::Emoji { .. }) {
        "64"
    } else {
        "32"
    };
    let mut html = String::from("<p>");
    for segment in &segments {
        match segment {
            HtmlSegment::Text(value) => html.push_str(value),
            HtmlSegment::LineBreak => html.push_str("<br>"),
            HtmlSegment::Emoji { source_url, token } => {
                html.push_str(&format!(
                    r#"<img data-mx-emoticon="" src="{}" alt="{}" title="{}" height="{}" width="{}">"#,
                    source_url, token, token, emoji_height, emoji_height
                ));
            }
        }
    }
    html.push_str("</p>");
    Some(html)
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

#[cfg(test)]
mod tests {
    use super::{
        build_display_formatted_body_from_custom_emoji, build_formatted_body_from_custom_emoji,
    };
    use crate::messages::types::MatrixPickerCustomEmoji;

    fn picker_emoji(shortcode: &str) -> MatrixPickerCustomEmoji {
        MatrixPickerCustomEmoji {
            name: String::from("Wave"),
            shortcodes: vec![shortcode.to_owned()],
            url: String::from("matrix-media://localhost/wave"),
            source_url: String::from("mxc://media.example.org/wave"),
            category: None,
        }
    }

    #[test]
    fn formats_custom_emoji_when_input_shortcode_case_differs() {
        let html = build_formatted_body_from_custom_emoji("Hello :WAVE:", &[picker_emoji("wave")])
            .expect("expected formatted custom emoji html");

        assert!(html.contains("<img data-mx-emoticon"));
        assert!(html.contains("src=\"mxc://media.example.org/wave\""));
    }

    #[test]
    fn formats_custom_emoji_when_picker_shortcode_has_uppercase() {
        let html = build_formatted_body_from_custom_emoji("Hello :wave:", &[picker_emoji("WAVE")])
            .expect("expected formatted custom emoji html");

        assert!(html.contains("<img data-mx-emoticon"));
        assert!(html.contains("src=\"mxc://media.example.org/wave\""));
    }

    #[test]
    fn display_formatted_body_uses_resolved_image_url() {
        let emoji = MatrixPickerCustomEmoji {
            name: String::from("Camera"),
            shortcodes: vec![String::from("camera")],
            url: String::from("asset://localhost/%2Fhome%2Flux%2F.cache%2Feu.luxuride.singularity%2Fmedia-cache%2Fimg-912143c7a4e8d624.bin"),
            source_url: String::from("mxc://matrix.luxuride.eu/LdbvMTwIEbZMgJmDKaXRRvlx"),
            category: None,
        };

        let html = build_display_formatted_body_from_custom_emoji(":camera:", &[emoji])
            .expect("expected display formatted body html");

        assert!(html.contains("src=\"asset://localhost/%2Fhome%2Flux%2F.cache%2Feu.luxuride.singularity%2Fmedia-cache%2Fimg-912143c7a4e8d624.bin\""));
    }
}
