use tauri::Manager;

mod auth;
mod messages;
mod room_updates;
mod rooms;
mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(auth::AuthState::default())
        .setup(|app| {
            let trigger_state = room_updates::start_room_update_worker(app.handle().clone());
            app.manage(trigger_state);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            auth::matrix_start_oauth,
            auth::matrix_complete_oauth,
            auth::matrix_session_status,
            auth::matrix_logout,
            rooms::matrix_get_chats,
            messages::matrix_get_chat_messages,
            room_updates::matrix_trigger_room_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
