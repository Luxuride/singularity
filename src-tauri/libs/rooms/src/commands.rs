use std::sync::Arc;

use auth::AuthState;
use storage::AppDb;
use types::rooms::{
    MatrixGetChatNavigationRequest, MatrixGetChatNavigationResponse, MatrixGetChatsResponse,
    MatrixGetRoomImageRequest, MatrixGetRoomImageResponse, MatrixJoinRoomRequest,
    MatrixJoinRoomResponse, MatrixSetRootSpaceOrderRequest, MatrixSetRootSpaceOrderResponse,
    MatrixTriggerRoomUpdateRequest, MatrixTriggerRoomUpdateResponse,
};
use types::{Paths, RoomUpdateTriggerState};

use crate::image::{self, has_stale_cached_chat_media};
use crate::navigation::{build_navigation_response, orderable_root_space_ids};
use crate::persistence::{collect_and_store_chats, load_cached_chats};

pub async fn get_chats(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &Arc<AuthState>,
    trigger_state: &RoomUpdateTriggerState,
) -> Result<MatrixGetChatsResponse, String> {
    let mut chats = load_cached_chats(app_db)?;

    if let Some(cached_chats) = chats.as_deref() {
        if has_stale_cached_chat_media(&MatrixGetChatsResponse {
            chats: cached_chats.to_vec(),
        }) {
            let client = auth_state.restore_client_and_get(paths, app_db).await?;
            let local_chats = collect_and_store_chats(app_db, &client).await;
            if !local_chats.is_empty() {
                chats = Some(local_chats);
            }
        }
    } else {
        let client = auth_state.restore_client_and_get(paths, app_db).await?;
        let local_chats = collect_and_store_chats(app_db, &client).await;
        if !local_chats.is_empty() {
            chats = Some(local_chats);
        }
    }

    let _ = trigger_state.enqueue_refresh(None, false);

    Ok(MatrixGetChatsResponse {
        chats: chats.unwrap_or_default(),
    })
}

pub fn get_chat_navigation(
    request: Option<MatrixGetChatNavigationRequest>,
    app_db: &Arc<AppDb>,
) -> Result<MatrixGetChatNavigationResponse, String> {
    let payload = request.unwrap_or_default();
    let chats = load_cached_chats(app_db)?.unwrap_or_default();
    let saved_root_space_ids = app_db.load_root_space_order()?;
    let saved_root_space_ids =
        (!saved_root_space_ids.is_empty()).then_some(saved_root_space_ids.as_slice());

    Ok(build_navigation_response(
        &chats,
        saved_root_space_ids,
        payload.root_space_id.as_deref(),
        payload.selected_room_id.as_deref(),
    ))
}

pub fn set_root_space_order(
    request: MatrixSetRootSpaceOrderRequest,
    app_db: &Arc<AppDb>,
) -> Result<MatrixSetRootSpaceOrderResponse, String> {
    let chats = app_db.load_cached_chats()?.unwrap_or_default();
    let orderable_root_space_ids = orderable_root_space_ids(&chats);

    let mut seen = std::collections::HashSet::new();
    let mut requested_root_space_ids = Vec::with_capacity(request.root_space_ids.len());

    for root_space_id in request.root_space_ids {
        if !orderable_root_space_ids.contains(&root_space_id) {
            return Err(format!("Unknown root space id: {root_space_id}"));
        }

        if !seen.insert(root_space_id.clone()) {
            return Err(format!("Duplicate root space id: {root_space_id}"));
        }

        requested_root_space_ids.push(root_space_id);
    }

    app_db.store_root_space_order(&requested_root_space_ids)?;

    Ok(MatrixSetRootSpaceOrderResponse {
        root_space_ids: requested_root_space_ids,
    })
}

pub fn trigger_room_update(
    request: Option<MatrixTriggerRoomUpdateRequest>,
    trigger_state: &RoomUpdateTriggerState,
) -> Result<MatrixTriggerRoomUpdateResponse, String> {
    let payload = request.unwrap_or_default();
    let _ =
        trigger_state.enqueue_refresh(payload.selected_room_id, payload.include_selected_messages);

    Ok(MatrixTriggerRoomUpdateResponse { queued: true })
}

pub async fn get_room_image(
    request: MatrixGetRoomImageRequest,
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &Arc<AuthState>,
    event_sink: &Arc<dyn types::EventSink>,
) -> Result<MatrixGetRoomImageResponse, String> {
    image::get_room_image(request, paths, app_db, auth_state, event_sink).await
}

pub async fn join_room(
    request: MatrixJoinRoomRequest,
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &Arc<AuthState>,
) -> Result<MatrixJoinRoomResponse, String> {
    crate::join::join_room(request, paths, app_db, auth_state).await
}
