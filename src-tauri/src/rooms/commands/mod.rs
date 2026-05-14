mod image;
mod join;
mod leave;
mod navigation;
mod preview;
mod space;
mod updates;

use tauri::{AppHandle, State};

use crate::auth::AuthState;
use crate::db::AppDb;

use super::types::{
    MatrixGetChatNavigationRequest, MatrixGetChatNavigationResponse, MatrixGetChatsResponse,
    MatrixGetRoomImageRequest, MatrixGetRoomImageResponse, MatrixGetRoomPreviewRequest,
    MatrixGetRoomPreviewResponse, MatrixGetSpaceChildIdsRequest,
    MatrixGetSpaceChildIdsResponse, MatrixJoinRoomRequest, MatrixJoinRoomResponse,
    MatrixLeaveRoomRequest, MatrixLeaveRoomResponse, MatrixLeaveRoomsRequest,
    MatrixLeaveRoomsResponse, MatrixSetRootSpaceOrderRequest, MatrixSetRootSpaceOrderResponse,
};
use super::{
    MatrixTriggerRoomUpdateRequest, MatrixTriggerRoomUpdateResponse, RoomUpdateTriggerState,
};

#[tauri::command]
pub async fn matrix_join_room(
    request: MatrixJoinRoomRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixJoinRoomResponse, String> {
    join::join_room(request, auth_state, app_handle).await
}

#[tauri::command]
pub async fn matrix_leave_room(
    request: MatrixLeaveRoomRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixLeaveRoomResponse, String> {
    leave::leave_room(request, auth_state, app_handle).await
}

#[tauri::command]
pub async fn matrix_leave_rooms(
    request: MatrixLeaveRoomsRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixLeaveRoomsResponse, String> {
    leave::leave_rooms(request, auth_state, app_handle).await
}

#[tauri::command]
pub async fn matrix_get_chats(
    auth_state: State<'_, AuthState>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatsResponse, String> {
    updates::get_chats(&auth_state, &trigger_state, &app_handle).await
}

#[tauri::command]
pub fn matrix_get_chat_navigation(
    request: Option<MatrixGetChatNavigationRequest>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatNavigationResponse, String> {
    updates::get_chat_navigation_joined(request, &app_handle)
}

#[tauri::command]
pub fn matrix_get_chat_navigation_with_unjoined(
    request: Option<MatrixGetChatNavigationRequest>,
    app_handle: AppHandle,
) -> Result<MatrixGetChatNavigationResponse, String> {
    updates::get_chat_navigation_with_unjoined(request, &app_handle)
}

#[tauri::command]
pub fn matrix_set_root_space_order(
    request: MatrixSetRootSpaceOrderRequest,
    app_db: State<'_, AppDb>,
) -> Result<MatrixSetRootSpaceOrderResponse, String> {
    updates::set_root_space_order(request, &app_db)
}

#[tauri::command]
pub async fn matrix_trigger_room_update(
    request: Option<MatrixTriggerRoomUpdateRequest>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<MatrixTriggerRoomUpdateResponse, String> {
    updates::trigger_room_update(request, &trigger_state)
}

#[tauri::command]
pub async fn matrix_get_room_image(
    request: MatrixGetRoomImageRequest,
    auth_state: State<'_, AuthState>,
    app_db: State<'_, AppDb>,
    app_handle: AppHandle,
) -> Result<MatrixGetRoomImageResponse, String> {
    image::get_room_image(request, &auth_state, &app_db, &app_handle).await
}

#[tauri::command]
pub async fn matrix_get_room_preview(
    request: MatrixGetRoomPreviewRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetRoomPreviewResponse, String> {
    preview::get_room_preview(request, auth_state, app_handle).await
}

#[tauri::command]
pub async fn matrix_get_space_child_ids(
    request: MatrixGetSpaceChildIdsRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixGetSpaceChildIdsResponse, String> {
    space::get_space_child_ids(request, auth_state, app_handle).await
}
