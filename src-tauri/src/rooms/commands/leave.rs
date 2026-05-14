use matrix_sdk::ruma::OwnedRoomId;
use std::convert::TryFrom;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::rooms::types::{
    MatrixLeaveRoomRequest, MatrixLeaveRoomResponse, MatrixLeaveRoomsRequest,
    MatrixLeaveRoomsResponse,
};

pub async fn leave_room(
    request: MatrixLeaveRoomRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixLeaveRoomResponse, String> {
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let room_id = OwnedRoomId::try_from(request.room_id.as_str())
        .map_err(|e| format!("Invalid room id: {e}"))?;

    let Some(room) = client.get_room(&room_id) else {
        return Err(String::from("Room is not available to leave"));
    };

    room.leave()
        .await
        .map_err(|e| format!("Failed to leave room: {e}"))?;

    Ok(MatrixLeaveRoomResponse {
        room_id: request.room_id,
    })
}

pub async fn leave_rooms(
    request: MatrixLeaveRoomsRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixLeaveRoomsResponse, String> {
    if request.room_ids.is_empty() {
        return Err(String::from("No rooms provided to leave"));
    }

    let client = auth_state.restore_client_and_get(&app_handle).await?;
    let mut left = Vec::with_capacity(request.room_ids.len());

    for room_id_raw in request.room_ids {
        let room_id = OwnedRoomId::try_from(room_id_raw.as_str())
            .map_err(|e| format!("Invalid room id: {e}"))?;

        let Some(room) = client.get_room(&room_id) else {
            return Err(String::from("Room is not available to leave"));
        };

        room.leave()
            .await
            .map_err(|e| format!("Failed to leave room: {e}"))?;

        left.push(room_id_raw);
    }

    Ok(MatrixLeaveRoomsResponse { room_ids: left })
}
