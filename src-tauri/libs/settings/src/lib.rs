//! Settings domain: media storage settings (asset vs. matrix-media).
//!
//! Tauri-free: functions take `&Paths` instead of an `AppHandle`. The media
//! settings file is stored at `Paths::data_file(APP_MEDIA_SETTINGS_FILE)`.

use std::fs;

use assets::{media_storage_mode, set_media_storage_mode, MediaStorageMode};
use types::settings::{
    MatrixGetMediaSettingsResponse, MatrixSetMediaSettingsRequest, MatrixSetMediaSettingsResponse,
};
use types::storage_keys;
use types::Paths;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedMediaSettings {
    media_storage_mode: String,
}

/// Load the persisted media storage mode and apply it to the assets runtime.
pub fn initialize_media_storage_mode(paths: &Paths) -> Result<(), String> {
    let mode = load_media_storage_mode(paths)?;
    set_media_storage_mode(mode);
    Ok(())
}

pub fn load_media_storage_mode(paths: &Paths) -> Result<MediaStorageMode, String> {
    let path = media_settings_path(paths);

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

pub fn persist_media_storage_mode(paths: &Paths, mode: MediaStorageMode) -> Result<(), String> {
    let serialized = match mode {
        MediaStorageMode::InMemory => "memory",
        MediaStorageMode::AssetStorage => "asset",
    };

    let path = media_settings_path(paths);
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

fn media_settings_path(paths: &Paths) -> std::path::PathBuf {
    paths.data_file(storage_keys::APP_MEDIA_SETTINGS_FILE)
}

pub fn current_media_settings() -> MatrixGetMediaSettingsResponse {
    MatrixGetMediaSettingsResponse {
        use_asset_storage: matches!(media_storage_mode(), MediaStorageMode::AssetStorage),
    }
}

/// Read the current media settings, persisting the loaded mode so the file is
/// in sync with the runtime.
pub fn get_media_settings(paths: &Paths) -> Result<MatrixGetMediaSettingsResponse, String> {
    let mode = load_media_storage_mode(paths)?;
    persist_media_storage_mode(paths, mode)?;
    Ok(current_media_settings())
}

/// Apply a new media storage mode and persist it.
pub fn set_media_settings(
    paths: &Paths,
    request: MatrixSetMediaSettingsRequest,
) -> Result<MatrixSetMediaSettingsResponse, String> {
    let mode = if request.use_asset_storage {
        MediaStorageMode::AssetStorage
    } else {
        MediaStorageMode::InMemory
    };

    persist_media_storage_mode(paths, mode)?;

    Ok(MatrixSetMediaSettingsResponse {
        use_asset_storage: request.use_asset_storage,
    })
}
