use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use tauri::{AppHandle, Manager};

pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data directory: {error}"))
}

pub fn app_data_file(app: &AppHandle, file_name: &str) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join(file_name))
}

pub fn get_or_create_keyring_secret(
    app: &AppHandle,
    service_name: &str,
    account_name: &str,
    bytes_len: usize,
) -> Result<String, String> {
    let use_file_fallback = std::env::var_os("FLATPAK_ID").is_some();

    let entry = keyring::Entry::new(service_name, account_name)
        .map_err(|error| format!("Failed to access keychain entry: {error}"))?;

    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => return Ok(secret),
        Ok(_) => {
            return Err(String::from(
                "Keychain returned an empty app database secret",
            ));
        }
        Err(keyring::Error::NoEntry) => {}
        Err(keyring::Error::NoStorageAccess(error)) => {
            if use_file_fallback {
                log::warn!(
                    "Keychain storage unavailable, falling back to file secret storage: {error}"
                );
                return get_or_create_file_secret(app, account_name, bytes_len);
            }

            return Err(format!(
                "Failed to access keychain storage for app database secret: {error}"
            ));
        }
        Err(error) => {
            if use_file_fallback {
                log::warn!("Keychain read failed, falling back to file secret storage: {error}");
                return get_or_create_file_secret(app, account_name, bytes_len);
            }

            return Err(format!(
                "Failed to read app database secret from keychain: {error}"
            ));
        }
    }

    let mut secret_bytes = vec![0_u8; bytes_len.max(32)];
    let mut rng = rand::rngs::OsRng;
    rand::RngCore::fill_bytes(&mut rng, &mut secret_bytes);
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, secret_bytes);

    match entry.set_password(&encoded) {
        Ok(()) => {}
        Err(keyring::Error::NoStorageAccess(error)) => {
            if use_file_fallback {
                log::warn!(
                    "Keychain storage unavailable while storing secret; falling back to file storage: {error}"
                );
                return get_or_create_file_secret(app, account_name, bytes_len);
            }

            return Err(format!(
                "Failed to access keychain storage for app database secret: {error}"
            ));
        }
        Err(error) => {
            if use_file_fallback {
                log::warn!(
                    "Failed to store keychain secret, falling back to file storage: {error}"
                );
                return get_or_create_file_secret(app, account_name, bytes_len);
            }

            return Err(format!(
                "Failed to store app database secret in keychain: {error}"
            ));
        }
    }

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
            return Ok(secret);
        }
    }

    let mut secret_bytes = vec![0_u8; bytes_len.max(32)];
    let mut rng = rand::rngs::OsRng;
    rand::RngCore::fill_bytes(&mut rng, &mut secret_bytes);
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, secret_bytes);

    std::fs::write(&path, &encoded)
        .map_err(|error| format!("Failed to persist fallback app database secret: {error}"))?;

    #[cfg(unix)]
    {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Failed to set fallback secret file permissions: {error}"))?;
    }

    Ok(encoded)
}
