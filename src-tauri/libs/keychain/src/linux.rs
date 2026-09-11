//! `oo7`-backed Secret Service implementation for Linux.
//!
//! `oo7` speaks D-Bus (or the Flatpak portal) and is the only backend that
//! works on Linux. Item names are `{service}:{account}`, matching the naming
//! used before this module was extracted.

use oo7::Secret;

fn item_name(service: &str, account: &str) -> String {
    format!("{service}:{account}")
}

pub(crate) async fn get(service: &str, account: &str) -> Result<Option<String>, String> {
    let name = item_name(service, account);
    let keyring = oo7::Keyring::new()
        .await
        .map_err(|error| format!("Failed to open keyring: {error}"))?;

    let items = keyring
        .search_items(&[("name", &name)])
        .await
        .map_err(|error| format!("Failed to search keyring: {error}"))?;

    let Some(item) = items.first() else {
        return Ok(None);
    };

    let secret = item
        .secret()
        .await
        .map_err(|error| format!("Failed to read keyring secret: {error}"))?;

    // `as_bytes` handles both `Text` and `Blob` variants; the stored value is a
    // base64-encoded string, so UTF-8 decoding is safe.
    let text = String::from_utf8(secret.as_bytes().to_vec())
        .map_err(|error| format!("Failed to decode keyring secret: {error}"))?;

    if text.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

pub(crate) async fn set(service: &str, account: &str, value: &str) -> Result<(), String> {
    let name = item_name(service, account);
    let attributes = &[("name", &name)];
    let keyring = oo7::Keyring::new()
        .await
        .map_err(|error| format!("Failed to open keyring: {error}"))?;

    keyring
        .create_item(&name, attributes, Secret::text(value), true)
        .await
        .map_err(|error| format!("Failed to write keyring secret: {error}"))
}

pub(crate) async fn delete(service: &str, account: &str) -> Result<(), String> {
    let name = item_name(service, account);
    let attributes = &[("name", &name)];
    let keyring = oo7::Keyring::new()
        .await
        .map_err(|error| format!("Failed to open keyring: {error}"))?;

    // `delete` is a no-op when nothing matches, so a missing secret is fine.
    keyring
        .delete(attributes)
        .await
        .map_err(|error| format!("Failed to delete keyring secret: {error}"))
}
