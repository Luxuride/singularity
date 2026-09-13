use std::sync::Arc;

use matrix_sdk::encryption::recovery::RecoveryState;
use url::Url;

use protocol::endpoints::normalize_homeserver_url;
use protocol::sync::sync_once_serialized;
use storage::AppDb;
use types::auth::{
    MatrixClearCacheExceptAuthResponse, MatrixCompleteOAuthResponse, MatrixLogoutResponse,
    MatrixPasswordLoginRequest, MatrixPasswordLoginResponse, MatrixRecoverWithKeyRequest,
    MatrixRecoverWithKeyResponse, MatrixRecoveryStatusResponse, MatrixSessionStatusResponse,
    MatrixStartOAuthRequest, MatrixStartOAuthResponse,
};
use types::{config, Paths};

use crate::persistence::{
    clear_app_cache, clear_app_cache_except_auth, clear_matrix_sdk_store, clear_persisted_session,
    persist_session, prepare_matrix_sdk_store, PersistedMatrixSession,
};
use crate::state::{wait_for_e2ee_initialization, AuthState, MatrixSession};
use crate::workers::start_session_persistence_watcher;

fn map_recovery_state(state: RecoveryState) -> String {
    match state {
        RecoveryState::Unknown => String::from("unknown"),
        RecoveryState::Enabled => String::from("enabled"),
        RecoveryState::Disabled => String::from("disabled"),
        RecoveryState::Incomplete => String::from("incomplete"),
    }
}

fn is_crypto_store_account_mismatch(error: &str) -> bool {
    error.contains("account in the store doesn't match the account in the constructor")
}

/// Begin an OAuth/SSO sign-in: build a client, fetch the SSO login URL, and store
/// the in-flight client for `complete_oauth`.
pub async fn start_oauth(
    paths: &Paths,
    auth_state: &AuthState,
    request: &MatrixStartOAuthRequest,
) -> Result<MatrixStartOAuthResponse, String> {
    let homeserver_url = normalize_homeserver_url(&request.homeserver_url)?;
    let store_path = prepare_matrix_sdk_store(paths)?;
    let client = matrix_sdk::Client::builder()
        .server_name_or_homeserver_url(homeserver_url)
        .sqlite_store(&store_path, None)
        .handle_refresh_tokens()
        .build()
        .await
        .map_err(|error| format!("Failed to initialize Matrix client: {error}"))?;

    let authorization_url = client
        .matrix_auth()
        .get_sso_login_url(config::CALLBACK_REDIRECT_URI, None)
        .await
        .map_err(|error| format!("Failed to construct Matrix SSO login URL: {error}"))?;

    auth_state.set_pending_client(client)?;

    let is_dev_container = std::env::var("SINGULARITY_DEV_CONTAINER")
        .map(|v| v == "true")
        .unwrap_or(false);

    Ok(MatrixStartOAuthResponse {
        authorization_url,
        redirect_uri: String::from(config::CALLBACK_REDIRECT_URI),
        is_dev_container,
    })
}

/// Finish an OAuth/SSO sign-in from the callback URL.
pub async fn complete_oauth(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    callback_url: &str,
) -> Result<MatrixCompleteOAuthResponse, String> {
    let callback_url =
        Url::parse(callback_url).map_err(|_| String::from("Callback URL is not a valid URL"))?;

    if callback_url.scheme() != config::CALLBACK_REDIRECT_SCHEME
        || callback_url.host_str() != Some(config::CALLBACK_REDIRECT_HOST)
    {
        return Err(String::from(
            "Callback URL has an unexpected redirect target",
        ));
    }

    let client = auth_state
        .take_pending_client()?
        .ok_or_else(|| String::from("Sign-in session expired. Please start sign-in again."))?;

    let parsed = match client
        .matrix_auth()
        .login_with_sso_callback(matrix_sdk::utils::UrlOrQuery::Url(callback_url))
        .map_err(|_| String::from("Callback URL is missing a valid loginToken"))?
        .initial_device_display_name("Singularity Desktop")
        .request_refresh_token()
        .send()
        .await
    {
        Ok(parsed) => parsed,
        Err(error) => {
            let error_text = error.to_string();

            if is_crypto_store_account_mismatch(&error_text) {
                log::warn!(
                    "Matrix login hit crypto-store account mismatch; resetting local auth and SDK store"
                );

                let _ = auth_state.clear_runtime_session();
                let _ = clear_persisted_session(app_db);
                let _ = clear_app_cache(app_db);
                let _ = clear_matrix_sdk_store(paths);

                return Err(String::from("Sign-in failed. Please start sign-in again."));
            }

            return Err(format!("Matrix login completion failed: {error_text}"));
        }
    };

    let homeserver_url = client.homeserver().to_string();
    let user_id = parsed.user_id.to_string();
    let device_id = parsed.device_id.to_string();

    let persisted_matrix_session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| String::from("Missing Matrix session after login"))?;
    persist_session(
        app_db,
        &PersistedMatrixSession::new(homeserver_url.clone(), persisted_matrix_session),
    )?;

    auth_state.set_authenticated(
        client.clone(),
        MatrixSession {
            homeserver_url: homeserver_url.clone(),
            user_id: user_id.clone(),
            device_id: device_id.clone(),
        },
    )?;

    start_session_persistence_watcher(app_db.clone(), client.clone());
    auth_state.fire_client_ready(&client);

    Ok(MatrixCompleteOAuthResponse {
        authenticated: true,
        homeserver_url,
        user_id,
        device_id,
    })
}

/// Sign in with username + password.
pub async fn password_login(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixPasswordLoginRequest,
) -> Result<MatrixPasswordLoginResponse, String> {
    let homeserver_url = normalize_homeserver_url(&request.homeserver_url)?;
    let username = request.username.trim();

    if username.is_empty() {
        return Err(String::from("Username is required"));
    }

    if request.password.is_empty() {
        return Err(String::from("Password is required"));
    }

    let store_path = prepare_matrix_sdk_store(paths)?;
    let client = matrix_sdk::Client::builder()
        .server_name_or_homeserver_url(homeserver_url.clone())
        .sqlite_store(&store_path, None)
        .handle_refresh_tokens()
        .build()
        .await
        .map_err(|error| format!("Failed to initialize Matrix client: {error}"))?;

    let parsed = match client
        .matrix_auth()
        .login_username(username, &request.password)
        .initial_device_display_name("Singularity Desktop")
        .request_refresh_token()
        .send()
        .await
    {
        Ok(parsed) => parsed,
        Err(error) => {
            let error_text = error.to_string();

            if is_crypto_store_account_mismatch(&error_text) {
                log::warn!(
                    "Matrix login hit crypto-store account mismatch; resetting local auth and SDK store"
                );

                let _ = auth_state.clear_runtime_session();
                let _ = clear_persisted_session(app_db);
                let _ = clear_app_cache(app_db);
                let _ = clear_matrix_sdk_store(paths);

                return Err(String::from("Sign-in failed. Please try again."));
            }

            return Err(String::from("Username/password sign-in failed"));
        }
    };

    let user_id = parsed.user_id.to_string();
    let device_id = parsed.device_id.to_string();

    let persisted_matrix_session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| String::from("Missing Matrix session after login"))?;
    persist_session(
        app_db,
        &PersistedMatrixSession::new(homeserver_url.clone(), persisted_matrix_session),
    )?;

    auth_state.set_authenticated(
        client.clone(),
        MatrixSession {
            homeserver_url: homeserver_url.clone(),
            user_id: user_id.clone(),
            device_id: device_id.clone(),
        },
    )?;

    start_session_persistence_watcher(app_db.clone(), client.clone());
    auth_state.fire_client_ready(&client);

    Ok(MatrixPasswordLoginResponse {
        authenticated: true,
        homeserver_url,
        user_id,
        device_id,
    })
}

/// Report the current session status, restoring from disk if needed.
pub async fn session_status(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<MatrixSessionStatusResponse, String> {
    auth_state
        .restore_client_from_disk_if_needed(paths, app_db)
        .await?;

    match auth_state.session()? {
        Some(session) => Ok(MatrixSessionStatusResponse {
            authenticated: true,
            homeserver_url: Some(session.homeserver_url),
            user_id: Some(session.user_id),
            device_id: Some(session.device_id),
        }),
        None => Ok(MatrixSessionStatusResponse {
            authenticated: false,
            homeserver_url: None,
            user_id: None,
            device_id: None,
        }),
    }
}

/// Report the E2EE recovery state.
pub async fn recovery_status(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<MatrixRecoveryStatusResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;
    wait_for_e2ee_initialization(&client).await;

    Ok(MatrixRecoveryStatusResponse {
        state: map_recovery_state(client.encryption().recovery().state()),
    })
}

/// Recover E2EE secrets from a recovery key, then sync and download backup keys.
pub async fn recover_with_key(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixRecoverWithKeyRequest,
) -> Result<MatrixRecoverWithKeyResponse, String> {
    let client = auth_state.restore_client_and_get(paths, app_db).await?;
    wait_for_e2ee_initialization(&client).await;

    let sync_timeout = std::time::Duration::from_secs(config::SYNC_TIMEOUT_SECONDS);

    client
        .encryption()
        .recovery()
        .recover(&request.recovery_key)
        .await
        .map_err(|error| format!("Failed to recover encryption secrets: {error}"))?;

    // First sync after recovery imports secrets and processes new to-device data.
    sync_once_serialized(
        &client,
        matrix_sdk::config::SyncSettings::default().timeout(sync_timeout),
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
        matrix_sdk::config::SyncSettings::default().timeout(sync_timeout),
    )
    .await
    .map_err(|error| format!("Failed to sync decrypted state after recovery: {error}"))?;

    Ok(MatrixRecoverWithKeyResponse {
        recovered: true,
        state: map_recovery_state(client.encryption().recovery().state()),
    })
}

/// Log out: clear the runtime session, log out remotely, and wipe local state.
pub async fn logout(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<MatrixLogoutResponse, String> {
    let client = auth_state.client().ok();

    auth_state.clear_runtime_session()?;

    if let Some(client) = client {
        if let Err(error) = client.logout().await {
            log::warn!("Matrix logout failed remotely, clearing local session anyway: {error}");
        }
    }

    clear_persisted_session(app_db)?;
    clear_app_cache(app_db)?;
    clear_matrix_sdk_store(paths)?;

    Ok(MatrixLogoutResponse { logged_out: true })
}

/// Clear the app cache while preserving the auth session.
pub fn clear_cache_except_auth(
    app_db: &Arc<AppDb>,
) -> Result<MatrixClearCacheExceptAuthResponse, String> {
    clear_app_cache_except_auth(app_db)?;

    Ok(MatrixClearCacheExceptAuthResponse { cleared: true })
}
