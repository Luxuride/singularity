use tauri::{AppHandle, State};
use url::Url;

use matrix_sdk::encryption::recovery::RecoveryState;

use crate::protocol::config;
use crate::protocol::endpoints::HomeserverEndpoints;
use crate::protocol::sync::sync_once_serialized;
use crate::verification::start_verification_state_watcher;
use crate::messages::MessageCacheState;

use super::persistence::{
    clear_matrix_sdk_store, clear_persisted_session, persist_session, prepare_matrix_sdk_store,
    PersistedMatrixSession,
};
use super::stateful_types::{
    MatrixCompleteOAuthRequest, MatrixCompleteOAuthResponse, MatrixLogoutResponse,
    MatrixRecoverWithKeyRequest, MatrixRecoverWithKeyResponse, MatrixRecoveryStatusResponse,
    MatrixSessionStatusResponse, MatrixStartOAuthRequest, MatrixStartOAuthResponse,
};
use super::{
    cross_process_lock_holder_name, start_session_persistence_watcher,
    wait_for_e2ee_initialization, AuthState, MatrixSession,
};

fn map_recovery_state(state: RecoveryState) -> String {
    match state {
        RecoveryState::Unknown => String::from("unknown"),
        RecoveryState::Enabled => String::from("enabled"),
        RecoveryState::Disabled => String::from("disabled"),
        RecoveryState::Incomplete => String::from("incomplete"),
    }
}

#[tauri::command]
pub async fn matrix_start_oauth(
    request: MatrixStartOAuthRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixStartOAuthResponse, String> {
    let endpoints = HomeserverEndpoints::from_raw(&request.homeserver_url)?;
    let homeserver_url = endpoints.homeserver_url().to_owned();
    let store_path = prepare_matrix_sdk_store(&app_handle)?;
    let client = matrix_sdk::Client::builder()
        .server_name_or_homeserver_url(homeserver_url)
        .sqlite_store(&store_path, None)
        .cross_process_store_locks_holder_name(cross_process_lock_holder_name())
        .handle_refresh_tokens()
        .build()
        .await
        .map_err(|error| format!("Failed to initialize Matrix client: {error}"))?;

    let authorization_url = client
        .matrix_auth()
        .get_sso_login_url(config::CALLBACK_REDIRECT_URI, None)
        .await
        .map_err(|error| format!("Failed to construct Matrix SSO login URL: {error}"))?;

    {
        let mut state = auth_state
            .inner
            .lock()
            .map_err(|_| String::from("Failed to acquire auth state lock"))?;
        state.pending_client = Some(client);
    }

    Ok(MatrixStartOAuthResponse {
        authorization_url,
        redirect_uri: String::from(config::CALLBACK_REDIRECT_URI),
    })
}

#[tauri::command]
pub async fn matrix_complete_oauth(
    request: MatrixCompleteOAuthRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixCompleteOAuthResponse, String> {
    let callback_url = Url::parse(&request.callback_url)
        .map_err(|_| String::from("Callback URL is not a valid URL"))?;

    let client = {
        let mut state = auth_state
            .inner
            .lock()
            .map_err(|_| String::from("Failed to acquire auth state lock"))?;
        state
            .pending_client
            .take()
            .ok_or_else(|| String::from("No login flow in progress. Start OAuth first."))?
    };

    let parsed = client
        .matrix_auth()
        .login_with_sso_callback(callback_url)
        .map_err(|_| String::from("Callback URL is missing a valid loginToken"))?
        .initial_device_display_name("Singularity Desktop")
        .request_refresh_token()
        .send()
        .await
        .map_err(|error| format!("Matrix login completion failed: {error}"))?;

    let homeserver_url = client.homeserver().to_string();

    if let Err(error) = client
        .encryption()
        .enable_cross_process_store_lock(client.cross_process_store_locks_holder_name().to_owned())
        .await
    {
        log::warn!("Failed to enable cross-process crypto store lock: {error}");
    }

    let persisted_matrix_session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| String::from("Missing Matrix session after login"))?;

    persist_session(
        &app_handle,
        &PersistedMatrixSession::new(homeserver_url.clone(), persisted_matrix_session),
    )?;

    {
        let mut state = auth_state
            .inner
            .lock()
            .map_err(|_| String::from("Failed to acquire auth state lock"))?;

        state.pending_client = None;
        state.client = Some(client.clone());
        state.session = Some(MatrixSession {
            homeserver_url: homeserver_url.clone(),
            user_id: parsed.user_id.to_string(),
            device_id: parsed.device_id.to_string(),
        });
    }

    wait_for_e2ee_initialization(&client).await;

    start_session_persistence_watcher(app_handle.clone(), client.clone());
    start_verification_state_watcher(app_handle.clone(), client);

    Ok(MatrixCompleteOAuthResponse {
        authenticated: true,
        homeserver_url,
        user_id: parsed.user_id.to_string(),
        device_id: parsed.device_id.to_string(),
    })
}

#[tauri::command]
pub async fn matrix_session_status(
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixSessionStatusResponse, String> {
    auth_state
        .restore_client_from_disk_if_needed(&app_handle)
        .await?;

    let state = auth_state
        .inner
        .lock()
        .map_err(|_| String::from("Failed to acquire auth state lock"))?;

    match &state.session {
        Some(session) => Ok(MatrixSessionStatusResponse {
            authenticated: true,
            homeserver_url: Some(session.homeserver_url.clone()),
            user_id: Some(session.user_id.clone()),
            device_id: Some(session.device_id.clone()),
        }),
        None => Ok(MatrixSessionStatusResponse {
            authenticated: false,
            homeserver_url: None,
            user_id: None,
            device_id: None,
        }),
    }
}

#[tauri::command]
pub async fn matrix_recovery_status(
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixRecoveryStatusResponse, String> {
    auth_state
        .restore_client_from_disk_if_needed(&app_handle)
        .await?;

    let client = auth_state.client()?;
    wait_for_e2ee_initialization(&client).await;

    Ok(MatrixRecoveryStatusResponse {
        state: map_recovery_state(client.encryption().recovery().state()),
    })
}

#[tauri::command]
pub async fn matrix_recover_with_key(
    request: MatrixRecoverWithKeyRequest,
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixRecoverWithKeyResponse, String> {
    auth_state
        .restore_client_from_disk_if_needed(&app_handle)
        .await?;

    let client = auth_state.client()?;
    wait_for_e2ee_initialization(&client).await;

    client
        .encryption()
        .recovery()
        .recover(&request.recovery_key)
        .await
        .map_err(|error| format!("Failed to recover encryption secrets: {error}"))?;

    // First sync after recovery imports secrets and processes new to-device data.
    sync_once_serialized(
        &client,
        matrix_sdk::config::SyncSettings::default()
            .timeout(std::time::Duration::from_secs(config::SYNC_TIMEOUT_SECONDS)),
    )
    .await
    .map_err(|error| format!("Failed to sync after recovery: {error}"))?;

    // If backups are active, proactively download room keys for encrypted joined
    // rooms so historical messages can decrypt immediately after recovery.
    if client.encryption().backups().are_enabled().await {
        let mut downloaded_rooms = 0usize;

        for room in client.joined_rooms() {
            let is_encrypted = room
                .latest_encryption_state()
                .await
                .map(|state| state.is_encrypted())
                .unwrap_or(false);

            if !is_encrypted {
                continue;
            }

            if let Err(error) = client
                .encryption()
                .backups()
                .download_room_keys_for_room(room.room_id())
                .await
            {
                log::warn!(
                    "Failed to download backup keys for room {} after recovery: {}",
                    room.room_id(),
                    error
                );
            } else {
                downloaded_rooms += 1;
            }
        }

        log::info!(
            "Recovered secrets and downloaded backup keys for {downloaded_rooms} encrypted rooms"
        );
    }

    // A second sync pass helps trigger re-decryption once keys are now local.
    sync_once_serialized(
        &client,
        matrix_sdk::config::SyncSettings::default()
            .timeout(std::time::Duration::from_secs(config::SYNC_TIMEOUT_SECONDS)),
    )
    .await
    .map_err(|error| format!("Failed to sync decrypted state after recovery: {error}"))?;

    Ok(MatrixRecoverWithKeyResponse {
        recovered: true,
        state: map_recovery_state(client.encryption().recovery().state()),
    })
}

#[tauri::command]
pub async fn matrix_logout(
    auth_state: State<'_, AuthState>,
    message_cache: State<'_, MessageCacheState>,
    app_handle: AppHandle,
) -> Result<MatrixLogoutResponse, String> {
    let client = auth_state.client().ok();

    auth_state.clear_runtime_session()?;

    if let Some(client) = client {
        if let Err(error) = client.logout().await {
            log::warn!("Matrix logout failed remotely, clearing local session anyway: {error}");
        }
    }

    clear_persisted_session(&app_handle)?;
    clear_matrix_sdk_store(&app_handle)?;
    message_cache.clear().await;

    Ok(MatrixLogoutResponse { logged_out: true })
}
