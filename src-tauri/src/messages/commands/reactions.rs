use log::info;
use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::protocol::sync::sync_once_serialized;
use crate::rooms::{RoomRefreshTrigger, RoomUpdateTriggerState};

use super::super::reactions::toggle_reaction_from_client;
use super::super::types::{MatrixToggleReactionRequest, MatrixToggleReactionResponse};

#[tauri::command]
pub async fn matrix_toggle_reaction(
    request: MatrixToggleReactionRequest,
    auth_state: State<'_, AuthState>,
    room_update_trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixToggleReactionResponse, String> {
    info!("matrix_toggle_reaction requested");
    let client = auth_state.restore_client_and_get(&app_handle).await?;

    sync_once_serialized(&client, matrix_sdk::config::SyncSettings::default())
        .await
        .map_err(|error| format!("Failed to sync Matrix before reaction toggle: {error}"))?;

    let room_id = request.room_id;
    let (added, event_id) = toggle_reaction_from_client(
        &client,
        room_id.as_str(),
        request.target_event_id.as_str(),
        request.key.as_str(),
    )
    .await?;

    let _ = room_update_trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: Some(room_id),
        include_selected_messages: true,
    });

    Ok(MatrixToggleReactionResponse { added, event_id })
}
