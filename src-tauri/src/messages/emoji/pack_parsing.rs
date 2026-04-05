use std::collections::BTreeSet;

use serde_json::Value;

pub(super) fn image_usage<'a>(content: &'a Value, image: &'a Value) -> BTreeSet<&'a str> {
    let mut usage = BTreeSet::new();

    if let Some(value) = image.get("usage").and_then(Value::as_str) {
        usage.insert(value);
    }

    if let Some(image_usage) = image.get("usage").and_then(Value::as_array) {
        for item in image_usage {
            if let Some(value) = item.as_str() {
                usage.insert(value);
            }
        }
    }

    if usage.is_empty() {
        if let Some(value) = content
            .get("pack")
            .and_then(|pack| pack.get("usage"))
            .and_then(Value::as_str)
        {
            usage.insert(value);
        }

        if let Some(pack_usage) = content
            .get("pack")
            .and_then(|pack| pack.get("usage"))
            .and_then(Value::as_array)
        {
            for item in pack_usage {
                if let Some(value) = item.as_str() {
                    usage.insert(value);
                }
            }
        }
    }

    usage
}

pub(super) fn usage_has_kind(usage: &BTreeSet<&str>, kind: &str) -> bool {
    usage.iter().any(|entry| {
        let normalized = entry.trim().to_ascii_lowercase();
        normalized == kind || normalized.ends_with(&format!(".{kind}")) || normalized.contains(kind)
    })
}

pub(super) fn fallback_usage_from_event_type(event_type: &str) -> Option<&'static str> {
    let normalized = event_type.to_ascii_lowercase();
    if normalized.contains("emote") {
        return Some("emoticon");
    }

    None
}

pub(super) fn unique_picker_name(
    used_names: &mut BTreeSet<String>,
    display_name: &str,
    shortcode: &str,
) -> String {
    let trimmed = display_name.trim();
    let base = if trimmed.is_empty() {
        shortcode
    } else {
        trimmed
    };
    let mut candidate = if trimmed.is_empty() {
        shortcode.to_owned()
    } else {
        trimmed.to_owned()
    };

    if !used_names.contains(&candidate.to_lowercase()) {
        used_names.insert(candidate.to_lowercase());
        return candidate;
    }

    let mut suffix = 2_u32;
    loop {
        candidate = format!("{base}-{suffix}");
        let lower = candidate.to_lowercase();
        if !used_names.contains(&lower) {
            used_names.insert(lower);
            return candidate;
        }
        suffix = suffix.saturating_add(1);
    }
}

pub(super) fn pack_media_url(image: &Value) -> Option<&str> {
    image.get("url").and_then(Value::as_str).or_else(|| {
        image
            .get("file")
            .and_then(|value| value.get("url"))
            .and_then(Value::as_str)
    })
}
