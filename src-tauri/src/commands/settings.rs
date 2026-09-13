//! Settings command adapters: media storage settings (asset vs. in-memory).
//! Delegates to the `settings` crate.

use tauri::State;

use types::settings::{
    MatrixGetMediaSettingsResponse, MatrixSetMediaSettingsRequest, MatrixSetMediaSettingsResponse,
};
use types::Paths;

#[tauri::command]
pub fn matrix_get_media_settings(
    paths: State<'_, Paths>,
) -> Result<MatrixGetMediaSettingsResponse, String> {
    settings::get_media_settings(&paths)
}

#[tauri::command]
pub fn matrix_set_media_settings(
    request: MatrixSetMediaSettingsRequest,
    paths: State<'_, Paths>,
) -> Result<MatrixSetMediaSettingsResponse, String> {
    settings::set_media_settings(&paths, request)
}
