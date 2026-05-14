use matrix_sdk::ruma::events::StateEventType;
use matrix_sdk::ruma::OwnedRoomId;
use std::collections::HashSet;
use std::convert::TryFrom;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::rooms::types::{
    MatrixGetSpaceChildIdsRequest, MatrixGetSpaceChildIdsResponse,
};

pub async fn get_space_child_ids(
    request: MatrixGetSpaceChildIdsRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetSpaceChildIdsResponse, String> {
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    let space_id = OwnedRoomId::try_from(request.space_id.as_str())
        .map_err(|error| format!("Invalid space ID: {error}"))?;

    let room = client
        .get_room(&space_id)
        .ok_or_else(|| String::from("Space not found in local client state"))?;

    let state_events = room
        .get_state_events(StateEventType::from("m.space.child"))
        .await
        .map_err(|error| format!("Failed to read space children: {error}"))?;

    let mut child_room_ids = HashSet::<String>::new();
    for raw_event in state_events {
        let Ok(event) = serde_json::to_value(&raw_event) else {
            continue;
        };

        let Some(child_room_id) = event.get("state_key").and_then(|value| value.as_str()) else {
            continue;
        };

        if !child_room_id.is_empty() {
            child_room_ids.insert(child_room_id.to_string());
        }
    }

    let mut child_room_ids = child_room_ids.into_iter().collect::<Vec<_>>();
    child_room_ids.sort_unstable();

    Ok(MatrixGetSpaceChildIdsResponse {
        space_id: space_id.to_string(),
        child_room_ids,
    })
}