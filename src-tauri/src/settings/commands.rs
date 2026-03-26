use tauri::State;

use crate::assets::image::MediaStorageMode;
use crate::db::AppDb;

use super::{
    current_media_settings, load_media_storage_mode, persist_media_storage_mode,
    MatrixGetMediaSettingsResponse, MatrixSetMediaSettingsRequest, MatrixSetMediaSettingsResponse,
};

#[tauri::command]
pub fn matrix_get_media_settings(
    app_db: State<'_, AppDb>,
) -> Result<MatrixGetMediaSettingsResponse, String> {
    let mode = load_media_storage_mode(&app_db)?;
    persist_media_storage_mode(&app_db, mode)?;
    Ok(current_media_settings())
}

#[tauri::command]
pub fn matrix_set_media_settings(
    request: MatrixSetMediaSettingsRequest,
    app_db: State<'_, AppDb>,
) -> Result<MatrixSetMediaSettingsResponse, String> {
    let mode = if request.use_asset_storage {
        MediaStorageMode::AssetStorage
    } else {
        MediaStorageMode::InMemory
    };

    persist_media_storage_mode(&app_db, mode)?;

    Ok(MatrixSetMediaSettingsResponse {
        use_asset_storage: request.use_asset_storage,
    })
}
