use tauri::{AppHandle, State};
use url::Url;

use crate::protocol::config;
use crate::protocol::endpoints::HomeserverEndpoints;

use super::persistence::{clear_persisted_session, persist_session, PersistedMatrixSession};
use super::stateful_types::{
    MatrixCompleteOAuthRequest, MatrixCompleteOAuthResponse, MatrixLogoutResponse,
    MatrixSessionStatusResponse, MatrixStartOAuthRequest, MatrixStartOAuthResponse,
};
use super::{AuthState, MatrixSession};

#[tauri::command]
pub async fn matrix_start_oauth(
    request: MatrixStartOAuthRequest,
    auth_state: State<'_, AuthState>,
) -> Result<MatrixStartOAuthResponse, String> {
    let endpoints = HomeserverEndpoints::from_raw(&request.homeserver_url)?;
    let homeserver_url = endpoints.homeserver_url().to_owned();
    let client = matrix_sdk::Client::builder()
        .server_name_or_homeserver_url(homeserver_url)
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
pub async fn matrix_logout(
    auth_state: State<'_, AuthState>,
    app_handle: AppHandle,
) -> Result<MatrixLogoutResponse, String> {
    let client = {
        let mut state = auth_state
            .inner
            .lock()
            .map_err(|_| String::from("Failed to acquire auth state lock"))?;

        state.pending_client = None;
        state.session = None;
        state.client.take()
    };

    if let Some(client) = client {
        client
            .logout()
            .await
            .map_err(|error| format!("Failed to logout Matrix session: {error}"))?;
    }

    clear_persisted_session(&app_handle)?;

    Ok(MatrixLogoutResponse { logged_out: true })
}
