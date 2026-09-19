//! Tauri binder: transport and setup only.
//!
//! This crate is the thin frontend adapter. All domain logic lives in the
//! workspace library crates (`auth`, `rooms`, `chat`, `settings`,
//! `verification`, `assets`, `storage`, `protocol`, `types`). The binder:
//!
//! - resolves `types::Paths` from the `AppHandle` during setup,
//! - initializes the app secret + encrypted database,
//! - clears the disk-backed media cache on startup,
//! - registers an `EventSink` that wraps `AppHandle::emit`,
//! - starts the room-update worker and the verification-state watcher (via the
//!   `AuthState::on_client_ready` hook),
//! - exposes `#[tauri::command]` adapters in `commands.rs` that delegate to the
//!   library crates.

use std::sync::Arc;

use tauri::Manager;

mod commands;

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
        .setup(|app| {
            let deep_link_registered = {
                #[cfg(any(windows, target_os = "linux"))]
                {
                    use tauri_plugin_deep_link::DeepLinkExt;

                    match app.deep_link().register_all() {
                        Ok(()) => true,
                        Err(error) => {
                            log::warn!("Skipping deep-link runtime registration: {error}");
                            false
                        }
                    }
                }

                #[cfg(not(any(windows, target_os = "linux")))]
                {
                    true
                }
            };

            let handle = app.handle().clone();
            let paths = commands::resolve_paths(&handle)?;
            assets::initialize_media_cache_dir(paths.cache_dir());
            // Media is disk-backed; clear any stale files from a previous
            // session so the cache does not accumulate on disk.
            assets::clear_media_cache();

            tauri::async_runtime::block_on(async {
                storage::secret::init_secret(
                    paths.data_dir(),
                    types::storage_keys::KEYCHAIN_SERVICE,
                    types::storage_keys::KEYCHAIN_APP_DB_KEY,
                    32,
                )
                .await
            })
            .expect("Failed to initialize app secret");

            let secret = storage::secret::get_secret().expect("App secret not initialized");
            let app_db = Arc::new(storage::AppDb::initialize(
                &paths.data_file(types::storage_keys::APP_DB_FILE),
                secret,
            )?);

            let event_sink: Arc<dyn types::EventSink> =
                Arc::new(commands::AppHandleEventSink::new(handle.clone()));
            let auth_state = Arc::new(auth::AuthState::default());
            auth_state.set_deep_link_registered(deep_link_registered);

            // Break the auth -> verification cycle: start the verification-state
            // watcher whenever a Matrix client becomes ready.
            {
                let sink = event_sink.clone();
                auth_state.set_on_client_ready(Box::new(move |client| {
                    verification::start_verification_state_watcher(sink.clone(), client);
                }));
            }

            let (trigger_state, room_worker) = rooms::start_room_update_worker(
                &paths,
                app_db.clone(),
                auth_state.clone(),
                event_sink.clone(),
            );

            // The rooms crate is Tauri-free and cannot assume a Tokio runtime is
            // running, so it returns the worker for the binder to spawn on
            // Tauri's managed async runtime.
            tauri::async_runtime::spawn(async move {
                room_worker.run().await;
            });

            app.manage(paths);
            app.manage(app_db);
            app.manage(auth_state);
            app.manage(event_sink);
            app.manage(trigger_state);
            app.manage(chat::MediaTranscodeCancellationState::default());
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
            commands::auth::matrix_start_oauth,
            commands::auth::matrix_complete_oauth,
            commands::auth::matrix_password_login,
            commands::auth::matrix_session_status,
            commands::auth::matrix_recovery_status,
            commands::auth::matrix_recover_with_key,
            commands::auth::matrix_logout,
            commands::auth::matrix_clear_cache_except_auth,
            commands::rooms::matrix_get_chats,
            commands::rooms::matrix_get_room_image,
            commands::rooms::matrix_get_chat_navigation,
            commands::rooms::matrix_join_room,
            commands::rooms::matrix_set_root_space_order,
            commands::rooms::matrix_trigger_room_update,
            commands::chat::matrix_get_chat_messages,
            commands::chat::matrix_stream_chat_messages,
            commands::chat::matrix_get_emoji_packs,
            commands::chat::matrix_get_user_avatar,
            commands::chat::matrix_send_chat_message,
            commands::chat::matrix_send_media_file,
            commands::chat::matrix_cancel_media_transcode,
            commands::chat::matrix_toggle_reaction,
            commands::chat::matrix_copy_image_to_clipboard,
            commands::chat::matrix_read_clipboard_text,
            commands::chat::matrix_resolve_video_url,
            commands::verification::matrix_own_verification_status,
            commands::verification::matrix_get_user_devices,
            commands::verification::matrix_request_device_verification,
            commands::verification::matrix_get_verification_flow,
            commands::verification::matrix_accept_verification_request,
            commands::verification::matrix_start_sas_verification,
            commands::verification::matrix_accept_sas_verification,
            commands::verification::matrix_confirm_sas_verification,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
