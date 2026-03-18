use std::path::PathBuf;

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
    _app: &AppHandle,
    service_name: &str,
    account_name: &str,
    bytes_len: usize,
) -> Result<String, String> {
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
            return Err(format!("Failed to access keychain storage for app database secret: {error}"));
        }
        Err(error) => {
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
            return Err(format!("Failed to access keychain storage for app database secret: {error}"));
        }
        Err(error) => {
            return Err(format!("Failed to store app database secret in keychain: {error}"));
        }
    }

    Ok(encoded)
}
