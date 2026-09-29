//! Sanitisation of remote `formatted_body` HTML.
//!
//! A room member — or a malicious or compromised homeserver — controls the
//! `formatted_body` of any event it sends. The frontend renders that HTML with
//! `{@html}` (`MessageBody.svelte`, `MessageComposer.svelte`), which assigns
//! `innerHTML` and performs no sanitisation of its own. Inside a Tauri webview
//! the IPC bridge is reachable from the same context, so unsanitised markup is
//! remote code execution in the desktop client rather than a cosmetic bug.
//!
//! Sanitising here — at the point where remote content is parsed, before it is
//! stored in the database and long before a Svelte component sees it — means
//! the stored cache is safe too, not just the live path.

use std::collections::HashSet;

/// Tags kept in a formatted body.
///
/// This is the subset of the Matrix rich-text spec that the app's own composer
/// emits, plus `<img>` for custom emoji. Anything else is dropped: a
/// `formatted_body` is untrusted input, so an unfamiliar tag is a possible
/// attack, not a formatting loss worth preserving.
const ALLOWED_TAGS: &[&str] = &[
    "p",
    "br",
    "em",
    "strong",
    "b",
    "i",
    "u",
    "s",
    "del",
    "code",
    "pre",
    "blockquote",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "a",
    "img",
    "span",
    "div",
    "sub",
    "sup",
    "hr",
];

/// Attributes kept per tag.
///
/// No `style` and no `on*`: inline event handlers are the whole attack, and
/// `style` allows arbitrary CSS that can spoof surrounding UI. `data-mx-*`
/// carries the custom-emoji metadata the app parses client-side, and
/// `data-mx-emoticon` is what `rewrite_img_tag` keys on to size emoji.
///
/// `class` is kept for the syntax-highlighting classes Matrix renderers emit
/// (`language-rust` and friends), which no app stylesheet targets.
const ALLOWED_ATTRIBUTES: &[&str] = &[
    "href",
    "title",
    "src",
    "alt",
    "width",
    "height",
    "start",
    "class",
    "data-mx-emoticon",
    "data-mx-color",
    "data-mx-bg-color",
];

/// URL schemes permitted in `href` and `src`.
///
/// `http`/`https` cover remote media and links, `mxc` the Matrix content URI
/// that the backend resolves to an `asset://` path, and `asset` the already
/// resolved local path. `data:` is deliberately excluded: it is not needed by
/// any code path here and is a common way to smuggle a payload past a naive
/// scheme check.
const ALLOWED_URL_SCHEMES: &[&str] = &["http", "https", "mxc", "asset", "mailto"];

fn ammonia_config() -> &'static ammonia::Builder<'static> {
    use std::sync::OnceLock;
    static CONFIG: OnceLock<ammonia::Builder<'static>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let mut builder = ammonia::Builder::new();
        builder
            .tags(ALLOWED_TAGS.iter().copied().collect::<HashSet<_>>())
            .generic_attributes(ALLOWED_ATTRIBUTES.iter().copied().collect::<HashSet<_>>())
            .url_schemes(ALLOWED_URL_SCHEMES.iter().copied().collect::<HashSet<_>>());
        builder
    })
}

/// Sanitise a remote `formatted_body`.
///
/// Returns `None` when nothing survives sanitisation, so a body consisting
/// only of disallowed markup is not rendered as an empty bubble: the frontend
/// falls back to the plain `body` when `formatted_body` is `None`.
pub fn sanitize_formatted_body(html: &str) -> Option<String> {
    if html.trim().is_empty() {
        return None;
    }

    let sanitized = ammonia_config().clean(html).to_string();
    let sanitized = sanitized.trim().to_owned();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_ordinary_matrix_formatting() {
        let html = "<p>Hello <strong>world</strong> and <em>others</em></p><ul><li>one</li></ul>";
        let sanitized = sanitize_formatted_body(html).unwrap();

        assert!(sanitized.contains("<strong>world</strong>"));
        assert!(sanitized.contains("<em>others</em>"));
        assert!(sanitized.contains("<li>one</li>"));
    }

    #[test]
    fn keeps_code_and_pre_blocks() {
        let html = "<pre><code class=\"language-rust\">fn main() {}</code></pre>";
        let sanitized = sanitize_formatted_body(html).unwrap();

        assert!(sanitized.contains("<pre>"));
        assert!(sanitized.contains("language-rust"), "class must survive: {sanitized}");
    }

    #[test]
    fn strips_script_and_its_contents() {
        let sanitized =
            sanitize_formatted_body("<p>ok</p><script>alert(document.cookie)</script>").unwrap();

        assert!(!sanitized.to_lowercase().contains("script"), "{sanitized}");
        assert!(!sanitized.contains("alert"), "{sanitized}");
        assert!(sanitized.contains("ok"));
    }

    #[test]
    fn strips_event_handler_attributes() {
        for payload in [
            "<img src=\"x\" onerror=\"steal()\">",
            "<img src=\"x\" onerror='steal()'>",
            "<img src=\"x\" ONERROR=\"steal()\">",
            "<div onmouseover=\"steal()\">hover</div>",
            "<a href=\"https://example.com\" onclick=\"steal()\">x</a>",
        ] {
            let sanitized = sanitize_formatted_body(payload).unwrap();
            let lowered = sanitized.to_lowercase();
            assert!(!lowered.contains("onerror"), "{payload} -> {sanitized}");
            assert!(!lowered.contains("onmouseover"), "{payload} -> {sanitized}");
            assert!(!lowered.contains("onclick"), "{payload} -> {sanitized}");
            assert!(!lowered.contains("steal()"), "{payload} -> {sanitized}");
        }
    }

    #[test]
    fn rejects_javascript_urls() {
        for payload in [
            "<a href=\"javascript:steal()\">click</a>",
            "<a href=\"JaVaScRiPt:steal()\">click</a>",
            "<a href=\"  javascript:steal()\">click</a>",
            "<img src=\"javascript:steal()\">",
        ] {
            let sanitized = sanitize_formatted_body(payload).unwrap();
            let lowered = sanitized.to_lowercase().replace(' ', "");
            assert!(!lowered.contains("javascript:"), "{payload} -> {sanitized}");
        }
    }

    #[test]
    fn rejects_other_dangerous_url_schemes() {
        for payload in [
            "<a href=\"data:text/html,<script>steal()</script>\">x</a>",
            "<img src=\"data:image/svg+xml,<svg onload=steal()>\">",
            "<a href=\"vbscript:steal()\">x</a>",
            "<a href=\"file:///etc/passwd\">x</a>",
        ] {
            let sanitized = sanitize_formatted_body(payload).unwrap();
            let lowered = sanitized.to_lowercase();
            assert!(!lowered.contains("data:"), "{payload} -> {sanitized}");
            assert!(!lowered.contains("vbscript:"), "{payload} -> {sanitized}");
            assert!(!lowered.contains("file:"), "{payload} -> {sanitized}");
        }
    }

    #[test]
    fn allows_the_schemes_the_app_actually_uses() {
        for payload in [
            "<a href=\"https://matrix.org\">m</a>",
            "<a href=\"http://localhost:8008\">h</a>",
            "<a href=\"mailto:someone@example.org\">m</a>",
            "<img src=\"mxc://example.org/abc123\">",
            "<img src=\"asset://local/photo.webp\">",
        ] {
            let sanitized = sanitize_formatted_body(payload).unwrap();
            assert!(
                !sanitized.contains("href=\"\""),
                "href was stripped: {payload} -> {sanitized}"
            );
        }

        let mxc = sanitize_formatted_body("<img src=\"mxc://example.org/abc123\">").unwrap();
        assert!(mxc.contains("mxc://example.org/abc123"), "{mxc}");
    }

    #[test]
    fn keeps_custom_emoji_metadata_and_dimensions() {
        let payload = "<img data-mx-emoticon=\":smile:\" src=\"mxc://example.org/e\" title=\":smile:\" alt=\":smile:\" width=\"32\" height=\"32\">";
        let sanitized = sanitize_formatted_body(payload).unwrap();

        assert!(sanitized.contains("data-mx-emoticon"), "{sanitized}");
        assert!(sanitized.contains("width=\"32\""), "{sanitized}");
        assert!(sanitized.contains("height=\"32\""), "{sanitized}");
        assert!(sanitized.contains("title=\""), "{sanitized}");
    }

    #[test]
    fn strips_iframes_objects_and_forms() {
        for payload in [
            "<iframe src=\"https://evil.example\"></iframe>",
            "<object data=\"https://evil.example\"></object>",
            "<form action=\"https://evil.example\"><input name=\"a\"></form>",
            "<embed src=\"https://evil.example\">",
        ] {
            // `None` is also an acceptable outcome: these tags carry no text
            // content, so once they are dropped nothing is left to render.
            let Some(sanitized) = sanitize_formatted_body(payload) else {
                continue;
            };
            let lowered = sanitized.to_lowercase();
            for tag in ["iframe", "object", "embed", "<form", "<input"] {
                assert!(!lowered.contains(tag), "{payload} -> {sanitized}");
            }
        }
    }

    #[test]
    fn strips_style_attributes() {
        let sanitized =
            sanitize_formatted_body("<p style=\"position:fixed;inset:0\">overlay</p>").unwrap();
        assert!(!sanitized.to_lowercase().contains("style"), "{sanitized}");
    }

    #[test]
    fn unbalanced_and_malformed_markup_does_not_panic() {
        for payload in [
            "<p>unclosed",
            "</p>stray close",
            "<<>><img src=\"x\"",
            "<p><p><p>deep",
            "plain text only",
            "<b><i>mismatched</b></i>",
        ] {
            let _ = sanitize_formatted_body(payload);
        }
    }

    #[test]
    fn empty_and_whitespace_bodies_become_none() {
        assert!(sanitize_formatted_body("").is_none());
        assert!(sanitize_formatted_body("   \n\t ").is_none());
    }

    #[test]
    fn a_body_of_only_forbidden_markup_becomes_none() {
        // The frontend treats None as "fall back to the plain-text body", which
        // is the right outcome rather than an empty rendered bubble.
        assert!(sanitize_formatted_body("<script>steal()</script>").is_none());
    }

    #[test]
    fn nested_and_repeated_payloads_stay_inert() {
        let payload = "<p><img src=\"x\" onerror=\"a()\"><a href=\"javascript:b()\" onmouseover=\"c()\">d</a></p><script>e()</script><p>keep</p>";
        let sanitized = sanitize_formatted_body(payload).unwrap();
        let lowered = sanitized.to_lowercase();

        assert!(!lowered.contains("onerror"));
        assert!(!lowered.contains("onmouseover"));
        assert!(!lowered.contains("javascript:"));
        assert!(!lowered.contains("script"));
        assert!(sanitized.contains("keep"));
    }
}
