use std::path::PathBuf;
use std::{fs, io};

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
    mirror_file_name: &str,
    bytes_len: usize,
) -> Result<String, String> {
    let entry = keyring::Entry::new(service_name, account_name)
        .map_err(|error| format!("Failed to access keychain entry: {error}"))?;

    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => {
            persist_secret_mirror(app, mirror_file_name, &secret)?;
            return Ok(secret);
        }
        Ok(_) => {
            return Err(String::from(
                "Keychain returned an empty app database secret",
            ));
        }
        Err(keyring::Error::NoEntry) => {
            if let Some(mirrored_secret) = load_secret_mirror(app, mirror_file_name)? {
                if let Err(error) = entry.set_password(&mirrored_secret) {
                    log::warn!("Failed to restore app database key into keychain: {error}");
                }
                return Ok(mirrored_secret);
            }
        }
        Err(keyring::Error::NoStorageAccess(_error)) => {
            if let Some(mirrored_secret) = load_secret_mirror(app, mirror_file_name)? {
                if let Err(error) = entry.set_password(&mirrored_secret) {
                    log::warn!("Failed to restore app database key into keychain: {error}");
                }
                return Ok(mirrored_secret);
            }
        }
        Err(error) => {
            if let Some(mirrored_secret) = load_secret_mirror(app, mirror_file_name)? {
                log::warn!(
                    "Failed to read app database secret from keychain; using local mirror: {error}"
                );
                return Ok(mirrored_secret);
            }
            return Err(format!("Failed to read app database secret from keychain: {error}"));
        }
    }

    let mut secret_bytes = vec![0_u8; bytes_len.max(32)];
    let mut rng = rand::rngs::OsRng;
    rand::RngCore::fill_bytes(&mut rng, &mut secret_bytes);
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, secret_bytes);

    match entry.set_password(&encoded) {
        Ok(()) => {}
        Err(keyring::Error::NoStorageAccess(error)) => {
            log::warn!(
                "Keychain storage unavailable; continuing with mirrored app database key: {error}"
            );
        }
        Err(error) => {
            return Err(format!("Failed to store app database secret in keychain: {error}"));
        }
    }

    persist_secret_mirror(app, mirror_file_name, &encoded)?;

    Ok(encoded)
}

fn load_secret_mirror(app: &AppHandle, mirror_file_name: &str) -> Result<Option<String>, String> {
    let path = app_data_file(app, mirror_file_name)?;
    if !path.exists() {
        return Ok(None);
    }

    let secret = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read app database key mirror: {error}"))?;
    let trimmed = secret.trim().to_owned();

    if trimmed.is_empty() {
        return Ok(None);
    }

    Ok(Some(trimmed))
}

fn persist_secret_mirror(
    app: &AppHandle,
    mirror_file_name: &str,
    secret: &str,
) -> Result<(), String> {
    let path = app_data_file(app, mirror_file_name)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create app data directory: {error}"))?;
    }

    fs::write(&path, secret)
        .map_err(|error| format!("Failed to write app database key mirror: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(
            |error: io::Error| format!("Failed to set app database key mirror permissions: {error}"),
        )?;
    }

    Ok(())
}
