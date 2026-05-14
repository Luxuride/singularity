use matrix_sdk::ruma::room::RoomType;
use matrix_sdk::ruma::{OwnedServerName, RoomOrAliasId};
use matrix_sdk::RoomState;
use std::convert::TryFrom;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::rooms::types::{
    MatrixGetRoomPreviewRequest, MatrixGetRoomPreviewResponse, MatrixRoomKind, MatrixRoomPreview,
};

pub async fn get_room_preview(
    request: MatrixGetRoomPreviewRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetRoomPreviewResponse, String> {
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let room_id_or_alias = <&RoomOrAliasId>::try_from(request.room_id_or_alias.as_str())
        .map_err(|error| format!("Invalid room ID or alias: {error}"))?;

    let server_names: Vec<OwnedServerName> = request
        .server_names
        .unwrap_or_default()
        .iter()
        .filter_map(|name| {
            <&matrix_sdk::ruma::ServerName>::try_from(name.as_str())
                .ok()
                .map(|parsed| parsed.to_owned())
        })
        .collect();

    let preview = client
        .get_room_preview(room_id_or_alias, server_names)
        .await
        .map_err(|error| format!("Failed to preview room: {error}"))?;

    let display_name = preview
        .name
        .or_else(|| preview.canonical_alias.as_ref().map(|alias| alias.to_string()))
        .unwrap_or_else(|| preview.room_id.to_string());

    let kind = if preview.room_type == Some(RoomType::Space) {
        MatrixRoomKind::Space
    } else {
        MatrixRoomKind::Room
    };

    let joined = matches!(preview.state, Some(RoomState::Joined));

    Ok(MatrixGetRoomPreviewResponse {
        room: MatrixRoomPreview {
            room_id: preview.room_id.to_string(),
            display_name,
            description: preview.topic,
            joined_members: preview.num_joined_members,
            kind,
            joined,
        },
    })
}
