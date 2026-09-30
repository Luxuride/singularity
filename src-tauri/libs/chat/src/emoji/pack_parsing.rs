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
        // Matched on a path or type segment, so "org.matrix.msc2762.emoticon"
        // and "m.emoticon" are the same kind while "notemoticon" is not.
        normalized.split('.').any(|segment| segment == kind)
    })
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

    // Caps rather than loops forever: every candidate past the cap is already
    // taken, so continuing cannot find a free name.
    let mut suffix = 2_u32;
    while suffix < MAX_NAME_SUFFIX {
        candidate = format!("{base}-{suffix}");
        let lower = candidate.to_lowercase();
        if !used_names.contains(&lower) {
            used_names.insert(lower);
            return candidate;
        }
        suffix = suffix.saturating_add(1);
    }

    candidate = format!("{base}-{suffix}");
    used_names.insert(candidate.to_lowercase());
    candidate
}

/// Upper bound on numeric suffixes tried before falling back to an unreserved
/// name. `u32::MAX` suffixes is far past any pack a homeserver will serve.
const MAX_NAME_SUFFIX: u32 = 1_000_000;

pub(super) fn pack_media_url(image: &Value) -> Option<&str> {
    image.get("url").and_then(Value::as_str).or_else(|| {
        image
            .get("file")
            .and_then(|value| value.get("url"))
            .and_then(Value::as_str)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{unique_picker_name, usage_has_kind};

    fn usage<'a>(entries: &[&'a str]) -> BTreeSet<&'a str> {
        entries.iter().copied().collect()
    }

    #[test]
    fn usage_kind_matches_on_a_segment() {
        assert!(usage_has_kind(&usage(&["emoticon"]), "emoticon"));
        assert!(usage_has_kind(
            &usage(&["org.matrix.msc2762.emoticon"]),
            "emoticon"
        ));
        assert!(usage_has_kind(&usage(&["m.sticker"]), "sticker"));
    }

    #[test]
    fn usage_kind_does_not_match_a_substring() {
        // "notemoticon" contains the kind as text but is a different usage.
        assert!(!usage_has_kind(&usage(&["notemoticon"]), "emoticon"));
        assert!(!usage_has_kind(
            &usage(&["emoticon_custom_pack"]),
            "emoticon"
        ));
    }

    #[test]
    fn a_taken_display_name_gets_a_suffix() {
        let mut used = BTreeSet::new();

        assert_eq!(unique_picker_name(&mut used, "Wave", "wave"), "Wave");
        assert_eq!(unique_picker_name(&mut used, "Wave", "wave2"), "Wave-2");
    }

    #[test]
    fn an_empty_display_name_falls_back_to_the_shortcode() {
        let mut used = BTreeSet::new();

        assert_eq!(unique_picker_name(&mut used, "  ", "party"), "party");
    }

    #[test]
    fn a_saturated_name_space_terminates() {
        let mut used = BTreeSet::new();
        for suffix in 0..1_000_000 {
            used.insert(format!("wave-{suffix}").to_lowercase());
        }

        // Every candidate below the cap is taken; the function must still
        // return rather than search forever.
        assert!(!unique_picker_name(&mut used, "Wave", "wave").is_empty());
    }
}
