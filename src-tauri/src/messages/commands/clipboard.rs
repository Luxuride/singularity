use std::borrow::Cow;

use arboard::{Clipboard, ImageData};
use log::info;

use super::super::types::MatrixCopyImageToClipboardRequest;
use crate::assets;

#[tauri::command]
pub async fn matrix_copy_image_to_clipboard(
    request: MatrixCopyImageToClipboardRequest,
) -> Result<(), String> {
    info!("matrix_copy_image_to_clipboard requested");

    let image_bytes = if let Some(bytes) =
        assets::image::load_media_bytes_from_resolved_url(request.image_url.as_str())
    {
        bytes
    } else if request.image_url.starts_with("http://") || request.image_url.starts_with("https://")
    {
        let response = reqwest::get(request.image_url.as_str())
            .await
            .map_err(|error| format!("Failed to fetch image URL: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Failed to fetch image URL with status {}",
                response.status()
            ));
        }

        response
            .bytes()
            .await
            .map_err(|error| format!("Failed to read image URL response: {error}"))?
            .to_vec()
    } else {
        return Err(String::from("Unsupported image URL scheme"));
    };

    if image_bytes.is_empty() {
        return Err(String::from("Image bytes are empty"));
    }

    let decoded = image::load_from_memory(image_bytes.as_slice())
        .map_err(|error| format!("Failed to decode image bytes: {error}"))?;

    let rgba = decoded.to_rgba8();
    let width =
        usize::try_from(rgba.width()).map_err(|_| String::from("Image width is out of range"))?;
    let height =
        usize::try_from(rgba.height()).map_err(|_| String::from("Image height is out of range"))?;

    let mut clipboard =
        Clipboard::new().map_err(|error| format!("Failed to initialize clipboard: {error}"))?;

    clipboard
        .set_image(ImageData {
            width,
            height,
            bytes: Cow::Owned(rgba.into_raw()),
        })
        .map_err(|error| format!("Failed to write image to clipboard: {error}"))
}
