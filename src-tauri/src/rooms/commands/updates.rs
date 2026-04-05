use tauri::AppHandle;

use crate::auth::AuthState;
use crate::rooms::types::{
    MatrixGetChatNavigationRequest, MatrixGetChatNavigationResponse, MatrixGetChatsResponse,
};
use crate::rooms::{
    MatrixTriggerRoomUpdateRequest, MatrixTriggerRoomUpdateResponse, RoomRefreshTrigger,
    RoomUpdateTriggerState,
};

use super::super::persistence::{collect_and_store_chats, load_cached_chats};
use super::image::has_stale_cached_chat_media;
use super::navigation::build_navigation_response;

pub(super) async fn get_chats(
    auth_state: &AuthState,
    trigger_state: &RoomUpdateTriggerState,
    app_handle: &AppHandle,
) -> Result<MatrixGetChatsResponse, String> {
    if let Some(cached_chats) = load_cached_chats(app_handle)? {
        let cached = MatrixGetChatsResponse {
            chats: cached_chats,
        };

        if has_stale_cached_chat_media(&cached) {
            let client = auth_state.restore_client_and_get(app_handle).await?;

            let local_chats = collect_and_store_chats(app_handle, &client).await;
            if !local_chats.is_empty() {
                let _ = trigger_state.enqueue(RoomRefreshTrigger {
                    selected_room_id: None,
                    include_selected_messages: false,
                });

                return Ok(MatrixGetChatsResponse { chats: local_chats });
            }
        }

        let _ = trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: None,
            include_selected_messages: false,
        });

        return Ok(cached);
    }

    let client = auth_state.restore_client_and_get(app_handle).await?;

    let local_chats = collect_and_store_chats(app_handle, &client).await;
    if !local_chats.is_empty() {
        let _ = trigger_state.enqueue(RoomRefreshTrigger {
            selected_room_id: None,
            include_selected_messages: false,
        });

        return Ok(MatrixGetChatsResponse { chats: local_chats });
    }

    let _ = trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: None,
        include_selected_messages: false,
    });

    Ok(MatrixGetChatsResponse { chats: local_chats })
}

pub(super) fn get_chat_navigation(
    request: Option<MatrixGetChatNavigationRequest>,
    app_handle: &AppHandle,
) -> Result<MatrixGetChatNavigationResponse, String> {
    let payload = request.unwrap_or_default();
    let chats = load_cached_chats(app_handle)?.unwrap_or_default();

    Ok(build_navigation_response(
        &chats,
        payload.root_space_id.as_deref(),
        payload.selected_room_id.as_deref(),
    ))
}

pub(super) fn trigger_room_update(
    request: Option<MatrixTriggerRoomUpdateRequest>,
    trigger_state: &RoomUpdateTriggerState,
) -> Result<MatrixTriggerRoomUpdateResponse, String> {
    let payload = request.unwrap_or_default();

    trigger_state.enqueue(RoomRefreshTrigger {
        selected_room_id: payload.selected_room_id,
        include_selected_messages: payload.include_selected_messages,
    })?;

    Ok(MatrixTriggerRoomUpdateResponse { queued: true })
}
