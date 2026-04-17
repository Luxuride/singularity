use tauri::Manager;

mod assets;
mod auth;
mod db;
mod messages;
mod protocol;
mod rooms;
mod settings;
mod storage;
mod verification;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {
            // Deep-link events are emitted by the plugin for forwarded launches.
        }));
    }

    builder
        .register_uri_scheme_protocol("matrix-media", |_ctx, request| {
            assets::image::handle_media_protocol_request(request)
        })
        .manage(auth::AuthState::default())
        .manage(messages::send::MediaTranscodeCancellationState::default())
        .setup(|app| {
            #[cfg(any(windows, target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;

                if let Err(error) = app.deep_link().register_all() {
                    log::warn!("Skipping deep-link runtime registration: {error}");
                }
            }

            assets::image::initialize_media_cache_dir(app.handle());

            let app_db = db::AppDb::initialize(app.handle())?;
            settings::initialize_media_storage_mode(app.handle())?;
            app.manage(app_db);

            let trigger_state = rooms::start_room_update_worker(app.handle().clone());
            app.manage(trigger_state);
            Ok(())
        })
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            auth::commands::matrix_start_oauth,
            auth::commands::matrix_complete_oauth,
            auth::commands::matrix_password_login,
            auth::commands::matrix_session_status,
            auth::commands::matrix_recovery_status,
            auth::commands::matrix_recover_with_key,
            auth::commands::matrix_logout,
            auth::commands::matrix_clear_cache_except_auth,
            rooms::commands::matrix_get_chats,
            rooms::commands::matrix_get_room_image,
            rooms::commands::matrix_get_chat_navigation,
            rooms::commands::matrix_set_root_space_order,
            rooms::commands::matrix_trigger_room_update,
            messages::commands::chat::matrix_get_chat_messages,
            messages::commands::chat::matrix_stream_chat_messages,
            messages::commands::emoji::matrix_get_emoji_packs,
            messages::commands::avatar::matrix_get_user_avatar,
            messages::commands::send::matrix_send_chat_message,
            messages::commands::send::matrix_send_media_file,
            messages::commands::send::matrix_cancel_media_transcode,
            messages::commands::reactions::matrix_toggle_reaction,
            messages::commands::clipboard::matrix_copy_image_to_clipboard,
            messages::commands::clipboard::matrix_read_clipboard_text,
            settings::commands::matrix_get_media_settings,
            settings::commands::matrix_set_media_settings,
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
