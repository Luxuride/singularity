use tauri::AppHandle;

use crate::assets::image::MediaStorageMode;

use super::{
    current_media_settings, load_media_storage_mode, persist_media_storage_mode,
    MatrixGetMediaSettingsResponse, MatrixSetMediaSettingsRequest, MatrixSetMediaSettingsResponse,
};

#[tauri::command]
pub fn matrix_get_media_settings(
    app_handle: AppHandle,
) -> Result<MatrixGetMediaSettingsResponse, String> {
    let mode = load_media_storage_mode(&app_handle)?;
    persist_media_storage_mode(&app_handle, mode)?;
    Ok(current_media_settings())
}

#[tauri::command]
pub fn matrix_set_media_settings(
    request: MatrixSetMediaSettingsRequest,
    app_handle: AppHandle,
) -> Result<MatrixSetMediaSettingsResponse, String> {
    let mode = if request.use_asset_storage {
        MediaStorageMode::AssetStorage
    } else {
        MediaStorageMode::InMemory
    };

    persist_media_storage_mode(&app_handle, mode)?;

    Ok(MatrixSetMediaSettingsResponse {
        use_asset_storage: request.use_asset_storage,
    })
}
