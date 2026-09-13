//! Rooms command adapters: chat list, navigation, room image, join, and update
//! triggers. Delegates to the `rooms` crate.

use std::sync::Arc;

use tauri::State;

use auth::AuthState;
use storage::AppDb;
use types::rooms::{
    MatrixGetChatNavigationRequest, MatrixGetChatNavigationResponse, MatrixGetChatsResponse,
    MatrixGetRoomImageRequest, MatrixGetRoomImageResponse, MatrixJoinRoomRequest,
    MatrixJoinRoomResponse, MatrixSetRootSpaceOrderRequest, MatrixSetRootSpaceOrderResponse,
    MatrixTriggerRoomUpdateRequest, MatrixTriggerRoomUpdateResponse,
};
use types::{Paths, RoomUpdateTriggerState};

#[tauri::command]
pub async fn matrix_get_chats(
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
    auth_state: State<'_, Arc<AuthState>>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<MatrixGetChatsResponse, String> {
    rooms::commands::get_chats(&paths, &app_db, &auth_state, &trigger_state).await
}

#[tauri::command]
pub fn matrix_get_chat_navigation(
    request: Option<MatrixGetChatNavigationRequest>,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixGetChatNavigationResponse, String> {
    rooms::commands::get_chat_navigation(request, &app_db)
}

#[tauri::command]
pub fn matrix_set_root_space_order(
    request: MatrixSetRootSpaceOrderRequest,
    app_db: State<'_, Arc<AppDb>>,
) -> Result<MatrixSetRootSpaceOrderResponse, String> {
    rooms::commands::set_root_space_order(request, &app_db)
}

#[tauri::command]
pub fn matrix_trigger_room_update(
    request: Option<MatrixTriggerRoomUpdateRequest>,
    trigger_state: State<'_, RoomUpdateTriggerState>,
) -> Result<MatrixTriggerRoomUpdateResponse, String> {
    rooms::commands::trigger_room_update(request, &trigger_state)
}

#[tauri::command]
pub async fn matrix_get_room_image(
    request: MatrixGetRoomImageRequest,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
    auth_state: State<'_, Arc<AuthState>>,
    event_sink: State<'_, Arc<dyn types::EventSink>>,
) -> Result<MatrixGetRoomImageResponse, String> {
    rooms::commands::get_room_image(request, &paths, &app_db, &auth_state, &event_sink).await
}

#[tauri::command]
pub async fn matrix_join_room(
    request: MatrixJoinRoomRequest,
    paths: State<'_, Paths>,
    app_db: State<'_, Arc<AppDb>>,
    auth_state: State<'_, Arc<AuthState>>,
) -> Result<MatrixJoinRoomResponse, String> {
    rooms::commands::join_room(request, &paths, &app_db, &auth_state).await
}
