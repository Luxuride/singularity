use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixGetMediaSettingsResponse {
    pub use_asset_storage: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetMediaSettingsRequest {
    pub use_asset_storage: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSetMediaSettingsResponse {
    pub use_asset_storage: bool,
}
