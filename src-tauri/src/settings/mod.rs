pub(crate) mod commands;
mod types;

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::Manager;

pub(crate) use types::{
    MatrixGetMediaSettingsResponse, MatrixSetMediaSettingsRequest, MatrixSetMediaSettingsResponse,
};

use crate::assets::image::{media_storage_mode, set_media_storage_mode, MediaStorageMode};
use crate::protocol::storage_keys;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedMediaSettings {
    media_storage_mode: String,
}

pub(crate) fn initialize_media_storage_mode<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
) -> Result<(), String> {
    let mode = load_media_storage_mode(app_handle)?;
    set_media_storage_mode(mode);
    Ok(())
}

pub(crate) fn load_media_storage_mode<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
) -> Result<MediaStorageMode, String> {
    let path = media_settings_path(app_handle)?;

    if !path.exists() {
        return Ok(MediaStorageMode::InMemory);
    }

    let bytes =
        fs::read(&path).map_err(|error| format!("Failed to read media settings file: {error}"))?;

    let persisted: PersistedMediaSettings = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Failed to decode media settings file: {error}"))?;

    match persisted.media_storage_mode.as_str() {
        "asset" => Ok(MediaStorageMode::AssetStorage),
        "memory" => Ok(MediaStorageMode::InMemory),
        _ => Ok(MediaStorageMode::InMemory),
    }
}

pub(crate) fn persist_media_storage_mode<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    mode: MediaStorageMode,
) -> Result<(), String> {
    let serialized = match mode {
        MediaStorageMode::InMemory => "memory",
        MediaStorageMode::AssetStorage => "asset",
    };

    let path = media_settings_path(app_handle)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create media settings directory: {error}"))?;
    }

    let payload = PersistedMediaSettings {
        media_storage_mode: serialized.to_string(),
    };

    let encoded = serde_json::to_vec_pretty(&payload)
        .map_err(|error| format!("Failed to encode media settings file: {error}"))?;

    fs::write(&path, encoded)
        .map_err(|error| format!("Failed to write media settings file: {error}"))?;

    set_media_storage_mode(mode);
    Ok(())
}

fn media_settings_path<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
) -> Result<PathBuf, String> {
    Ok(app_handle
        .path()
        .app_config_dir()
        .map_err(|error| format!("Failed to resolve app config directory: {error}"))?
        .join(storage_keys::APP_MEDIA_SETTINGS_FILE))
}

pub(crate) fn current_media_settings() -> MatrixGetMediaSettingsResponse {
    MatrixGetMediaSettingsResponse {
        use_asset_storage: matches!(media_storage_mode(), MediaStorageMode::AssetStorage),
    }
}
