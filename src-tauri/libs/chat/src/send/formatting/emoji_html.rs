use std::collections::HashMap;

use types::chat::MatrixPickerCustomEmoji;

enum HtmlSegment {
    Text(String),
    LineBreak,
    Emoji { source_url: String, token: String },
}

pub fn build_formatted_body_from_custom_emoji(
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

pub fn build_display_formatted_body_from_custom_emoji(
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
                    escape_html_attribute(source_url),
                    escape_html_attribute(token),
                    escape_html_attribute(token),
                    emoji_height,
                    emoji_height
                ));
            }
        }
    }
    html.push_str("</p>");
    Some(html)
}

/// Escape a string for use as HTML text content.
///
/// The composer body is plain text that this function interpolates into markup,
/// so every character that can start or end a tag or an entity has to be
/// escaped. Without this, typing `hi <img src=x onerror=...> :wave:` into the
/// composer would turn the user's own message into live markup in the
/// `{@html}` sink that renders it.
fn escape_html_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Escape a string for use inside a double-quoted HTML attribute value.
///
/// `"` is what terminates the attribute, so it must become an entity; without
/// it a value such as `x" onerror="alert(1)` breaks out of the attribute and
/// injects an event handler. `'` is escaped too so the output is also safe if a
/// renderer prefers single quotes.
fn escape_html_attribute(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            other => escaped.push(other),
        }
    }
    escaped
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
            segments.push(HtmlSegment::Text(escape_html_text(&value[start..idx])));
        }

        segments.push(HtmlSegment::LineBreak);
        // Advance *past* the newline. Leaving `start` on the newline makes the
        // trailing slice below re-emit it, rendering every line break twice.
        start = idx + ch.len_utf8();
    }

    if start < value.len() {
        segments.push(HtmlSegment::Text(escape_html_text(&value[start..])));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_display_formatted_body_from_custom_emoji, build_formatted_body_from_custom_emoji,
        escape_html_attribute, escape_html_text,
    };
    use types::chat::MatrixPickerCustomEmoji;

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

    #[test]
    fn escapes_markup_typed_into_the_composer() {
        // A formatted body is only produced when at least one emoji was
        // substituted, so the escaped text has to sit alongside a shortcode.
        let html = build_formatted_body_from_custom_emoji(
            "hi <img src=x onerror=\"alert(1)\"> <b>bold</b> & :wave: more",
            &[picker_emoji("wave")],
        )
        .expect("expected formatted body");

        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"));
        assert!(html.contains("&amp;"));
        assert!(!html.contains("<img src=x"));
        assert!(!html.contains("<b>bold</b>"));
    }

    #[test]
    fn escapes_a_source_url_that_tries_to_break_out_of_the_attribute() {
        // The display builder interpolates the resolved `url`; the send builder
        // interpolates `source_url`. Both go through the same escaper.
        let emoji = MatrixPickerCustomEmoji {
            name: String::from("Evil"),
            shortcodes: vec![String::from("evil")],
            url: String::from("matrix-media://localhost/x\" onerror=\"alert(1)"),
            source_url: String::from("mxc://media.example.org/x\" onerror=\"alert(1)"),
            category: None,
        };

        for html in [
            build_display_formatted_body_from_custom_emoji(":evil:", std::slice::from_ref(&emoji)),
            build_formatted_body_from_custom_emoji(":evil:", std::slice::from_ref(&emoji)),
        ] {
            let html = html.expect("expected formatted body");
            assert!(html.contains("&quot;"), "{html}");
            assert!(!html.contains("onerror=\"alert(1)\""), "{html}");
        }
    }

    #[test]
    fn a_single_newline_becomes_exactly_one_line_break() {
        let html =
            build_formatted_body_from_custom_emoji("one\ntwo :wave:", &[picker_emoji("wave")])
                .expect("expected formatted body");

        assert_eq!(html.matches("<br>").count(), 1, "{html}");
    }

    #[test]
    fn a_trailing_newline_is_not_re_emitted_as_text() {
        let html = build_formatted_body_from_custom_emoji("one :wave:\n", &[picker_emoji("wave")])
            .expect("expected formatted body");

        assert_eq!(html.matches("<br>").count(), 1, "{html}");
    }

    #[test]
    fn escapes_ampersands_in_text_without_double_escaping() {
        assert_eq!(escape_html_text("a & b"), "a &amp; b");
        assert_eq!(escape_html_text("&lt;"), "&amp;lt;");
    }

    #[test]
    fn escapes_quotes_in_attribute_values() {
        assert_eq!(escape_html_attribute("a\"b'c"), "a&quot;b&#x27;c");
    }
}
