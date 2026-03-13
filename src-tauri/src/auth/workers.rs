use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::protocol::config;

use super::persistence::{
    clear_matrix_sdk_store, clear_persisted_session, persist_session_from_client,
};
use super::AuthState;

fn is_invalid_refresh_token_error(message: &str) -> bool {
    message.contains("UnknownToken") || message.contains("refresh token does not exist")
}

pub(crate) async fn handle_unknown_token_error(
    app: &AppHandle,
    auth_state: &AuthState,
    client: &matrix_sdk::Client,
) -> Result<(), String> {
    log::warn!("Matrix request returned unknown token, attempting refresh token recovery");

    if let Err(error) = client.matrix_auth().refresh_access_token().await {
        let message = format!("Failed to refresh Matrix access token: {error}");

        if is_invalid_refresh_token_error(&message) {
            log::warn!("Matrix refresh token is invalid, clearing local session");
            auth_state.clear_runtime_session()?;
            clear_persisted_session(app)?;
            clear_matrix_sdk_store(app)?;
            return Ok(());
        }

        return Err(message);
    }

    persist_session_from_client(app, client)?;
    log::info!("Matrix access token refreshed successfully after unknown token response");

    Ok(())
}

pub fn start_token_rotation_worker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(config::TOKEN_ROTATION_INTERVAL_SECONDS));

        loop {
            interval.tick().await;

            if let Err(error) = run_token_rotation_pass(&app).await {
                log::warn!("Matrix token rotation failed: {error}");
            }
        }
    });
}

async fn run_token_rotation_pass(app: &AppHandle) -> Result<(), String> {
    let auth_state = app.state::<AuthState>();
    auth_state.restore_client_from_disk_if_needed(app).await?;

    let client = match auth_state.client() {
        Ok(client) => client,
        Err(_) => return Ok(()),
    };

    let has_refresh_token = client
        .session_tokens()
        .and_then(|tokens| tokens.refresh_token)
        .is_some();

    if !has_refresh_token {
        return Ok(());
    }

    if let Err(error) = client.matrix_auth().refresh_access_token().await {
        let message = format!("Failed to refresh Matrix access token: {error}");

        if is_invalid_refresh_token_error(&message) {
            log::warn!("Matrix refresh token is invalid, clearing local session");
            auth_state.clear_runtime_session()?;
            clear_persisted_session(app)?;
            clear_matrix_sdk_store(app)?;
            return Ok(());
        }

        return Err(message);
    }

    persist_session_from_client(app, &client)?;

    Ok(())
}
