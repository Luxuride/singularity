use tauri::Manager;

mod auth;
mod messages;
mod protocol;
mod rooms;
mod storage;
mod verification;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(auth::AuthState::default())
        .setup(|app| {
            auth::start_token_rotation_worker(app.handle().clone());
            let trigger_state = rooms::start_room_update_worker(app.handle().clone());
            app.manage(trigger_state);
            Ok(())
        })
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            auth::commands::matrix_start_oauth,
            auth::commands::matrix_complete_oauth,
            auth::commands::matrix_session_status,
            auth::commands::matrix_recovery_status,
            auth::commands::matrix_recover_with_key,
            auth::commands::matrix_logout,
            rooms::commands::matrix_get_chats,
            rooms::commands::matrix_trigger_room_update,
            messages::commands::matrix_get_chat_messages,
            verification::commands::matrix_own_verification_status,
            verification::commands::matrix_get_user_devices,
            verification::commands::matrix_request_device_verification,
            verification::commands::matrix_get_verification_flow,
            verification::commands::matrix_accept_verification_request,
            verification::commands::matrix_start_sas_verification,
            verification::commands::matrix_accept_sas_verification,
            verification::commands::matrix_confirm_sas_verification,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
