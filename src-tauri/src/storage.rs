use std::{path::PathBuf, sync::OnceLock};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use tauri::{AppHandle, Manager};

static SECRET: OnceLock<String> = OnceLock::new();

pub fn get_secret() -> Option<&'static str> {
    SECRET.get().map(String::as_str)
}

pub async fn init_secret(
    app: &AppHandle,
    service_name: &str,
    account_name: &str,
    bytes_len: usize,
) -> Result<(), String> {
    let secret = get_or_create_keyring_secret(app, service_name, account_name, bytes_len).await?;
    // Idempotent: some keychain paths may have already set SECRET internally
    SECRET.get_or_init(|| secret);
    Ok(())
}

pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))
}

pub fn app_data_file(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join(file_name))
}

pub async fn get_or_create_keyring_secret(
    app: &AppHandle,
    service_name: &str,
    account_name: &str,
    bytes_len: usize,
) -> Result<String, String> {
    let item_name = format!("{service_name}:{account_name}");
    let attributes = &[("name", &item_name)];

    let keychain = match oo7::Keyring::new().await {
        Ok(kc) => kc,
        Err(error) => {
            log::warn!("Keychain unavailable, falling back to file secret storage: {error}");
            return get_or_create_file_secret(app, account_name, bytes_len);
        }
    };

    match keychain.search_items(attributes).await {
        Ok(items) if !items.is_empty() => {
            if let Some(secret) = items.first() {
                let oo7::Secret::Text(secret_str) = &secret.secret().await.unwrap() else {
                    panic!();
                };
                if !secret_str.trim().is_empty() {
                    SECRET.set(secret_str.clone()).ok();
                    return Ok(secret_str.clone());
                }
            }
            return Err(String::from(
                "Keychain returned an empty app database secret",
            ));
        }
        Err(error) => {
            log::warn!("Keychain read failed, falling back to file secret storage: {error}");
            return get_or_create_file_secret(app, account_name, bytes_len);
        }
        Ok(_) => {
            // Keychain exists but no matching item — fall through to create one
        }
    }

    let mut secret_bytes = vec![0_u8; bytes_len.max(32)];
    let mut rng = rand::rngs::OsRng;
    rand::RngCore::fill_bytes(&mut rng, &mut secret_bytes);
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, secret_bytes);

    match keychain
        .create_item(&item_name, attributes, encoded.as_bytes(), true)
        .await
    {
        Ok(()) => {}
        Err(error) => {
            log::warn!("Failed to store keychain secret, falling back to file storage: {error}");
            return get_or_create_file_secret(app, account_name, bytes_len);
        }
    }

    SECRET.set(encoded.clone()).ok();
    Ok(encoded)
}

fn get_or_create_file_secret(
    app: &AppHandle,
    account_name: &str,
    bytes_len: usize,
) -> Result<String, String> {
    let file_name = format!("{account_name}.secret");
    let path = app_data_file(app, &file_name)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create secret storage directory: {error}"))?;
    }

    if let Ok(secret) = std::fs::read_to_string(&path) {
        if !secret.trim().is_empty() {
            SECRET.set(secret.clone()).ok();
            return Ok(secret);
        }
    }

    let mut secret_bytes = vec![0_u8; bytes_len.max(32)];
    let mut rng = rand::rngs::OsRng;
    rand::RngCore::fill_bytes(&mut rng, &mut secret_bytes);
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, secret_bytes);

    std::fs::write(&path, &encoded)
        .map_err(|error| format!("Failed to persist fallback app database secret: {error}"))?;

    SECRET.set(encoded.clone()).ok();

    #[cfg(unix)]
    {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Failed to set fallback secret file permissions: {error}"))?;
    }

    SECRET.set(encoded.clone()).ok();
    Ok(encoded)
}
