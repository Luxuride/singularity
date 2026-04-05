use serde_json::Value;

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

pub(super) fn root_category(content: &Value, fallback: Option<String>) -> Option<String> {
    content
        .get("pack")
        .and_then(|pack| pack.get("display_name"))
        .and_then(Value::as_str)
        .and_then(non_empty_trimmed)
        .or(fallback)
        .filter(|value| !value.trim().is_empty())
}

pub(super) fn nested_category_from_pack(
    pack_content: &Value,
    pack_id: &str,
    root_category: Option<String>,
) -> Option<String> {
    pack_content
        .get("pack")
        .and_then(|pack| pack.get("display_name"))
        .and_then(Value::as_str)
        .and_then(non_empty_trimmed)
        .or_else(|| non_empty_trimmed(pack_id))
        .or(root_category)
}

pub(super) fn nested_category_from_entry(
    entry_value: &Value,
    entry_key: &str,
    root_category: Option<String>,
) -> Option<String> {
    entry_value
        .get("pack")
        .and_then(|pack| pack.get("display_name"))
        .and_then(Value::as_str)
        .and_then(non_empty_trimmed)
        .or_else(|| non_empty_trimmed(entry_key))
        .or(root_category)
}

pub(super) fn referenced_category(
    pack_reference: &Value,
    pack_id: &str,
    root_category: Option<String>,
) -> Option<String> {
    pack_reference
        .get("pack")
        .and_then(|pack| pack.get("display_name"))
        .and_then(Value::as_str)
        .and_then(non_empty_trimmed)
        .or_else(|| non_empty_trimmed(pack_id))
        .or(root_category)
}
