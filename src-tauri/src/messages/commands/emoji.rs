use log::info;
use tauri::{AppHandle, State};

use crate::auth::AuthState;

use super::super::emoji::load_picker_assets_from_client;
use super::super::types::MatrixGetEmojiPacksResponse;

#[tauri::command]
pub async fn matrix_get_emoji_packs(
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetEmojiPacksResponse, String> {
    info!("matrix_get_emoji_packs requested");
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let custom_emoji = load_picker_assets_from_client(&client).await?;

    Ok(MatrixGetEmojiPacksResponse { custom_emoji })
}
