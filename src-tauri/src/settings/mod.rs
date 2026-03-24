pub(crate) mod commands;
mod types;

pub(crate) use types::{
    MatrixGetMediaSettingsResponse, MatrixSetMediaSettingsRequest, MatrixSetMediaSettingsResponse,
};

use crate::db::AppDb;
use crate::messages::{media_storage_mode, set_media_storage_mode, MediaStorageMode};
use crate::protocol::storage_keys;

pub(crate) fn initialize_media_storage_mode(app_db: &AppDb) -> Result<(), String> {
    let mode = load_media_storage_mode(app_db)?;
    set_media_storage_mode(mode);
    Ok(())
}

pub(crate) fn load_media_storage_mode(app_db: &AppDb) -> Result<MediaStorageMode, String> {
    let Some(raw) = app_db.get_setting(storage_keys::APP_SETTING_MEDIA_STORAGE_MODE)? else {
        return Ok(MediaStorageMode::InMemory);
    };

    match raw.as_str() {
        "asset" => Ok(MediaStorageMode::AssetStorage),
        "memory" => Ok(MediaStorageMode::InMemory),
        _ => Ok(MediaStorageMode::InMemory),
    }
}

pub(crate) fn persist_media_storage_mode(app_db: &AppDb, mode: MediaStorageMode) -> Result<(), String> {
    let serialized = match mode {
        MediaStorageMode::InMemory => "memory",
        MediaStorageMode::AssetStorage => "asset",
    };

    app_db.set_setting(storage_keys::APP_SETTING_MEDIA_STORAGE_MODE, serialized)?;
    set_media_storage_mode(mode);
    Ok(())
}

pub(crate) fn current_media_settings() -> MatrixGetMediaSettingsResponse {
    MatrixGetMediaSettingsResponse {
        use_asset_storage: matches!(media_storage_mode(), MediaStorageMode::AssetStorage),
    }
}
