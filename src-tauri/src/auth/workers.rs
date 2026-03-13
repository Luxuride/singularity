use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::protocol::config;

use super::persistence::persist_session_from_client;
use super::AuthState;

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

    client
        .matrix_auth()
        .refresh_access_token()
        .await
        .map_err(|error| format!("Failed to refresh Matrix access token: {error}"))?;

    persist_session_from_client(app, &client)?;

    Ok(())
}
