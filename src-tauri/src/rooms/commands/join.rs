use std::convert::TryFrom;
use matrix_sdk::ruma::{OwnedServerName, RoomOrAliasId};
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::rooms::types::{MatrixJoinRoomRequest, MatrixJoinRoomResponse};

pub async fn join_room(
    request: MatrixJoinRoomRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixJoinRoomResponse, String> {
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let room_id_or_alias = <&RoomOrAliasId>::try_from(request.room_id_or_alias.as_str())
        .map_err(|e| format!("Invalid room ID or alias: {e}"))?;

    let server_names: Vec<OwnedServerName> = request
        .server_names
        .unwrap_or_default()
        .iter()
        .filter_map(|s| <&matrix_sdk::ruma::ServerName>::try_from(s.as_str()).ok().map(|name| name.to_owned()))
        .collect();

    let response = client
        .join_room_by_id_or_alias(room_id_or_alias, &server_names)
        .await
        .map_err(|e| format!("Failed to join room: {e}"))?;

    Ok(MatrixJoinRoomResponse {
        room_id: response.room_id().to_string(),
    })
}
