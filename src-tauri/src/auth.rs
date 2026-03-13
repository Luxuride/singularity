use matrix_sdk::authentication::matrix::MatrixSession as SdkMatrixSession;
use matrix_sdk::store::RoomLoadSettings;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use matrix_sdk::Client;
use tauri::{AppHandle, Manager, State};
use url::Url;

use crate::storage;

const CALLBACK_REDIRECT_URI: &str = "http://127.0.0.1:8743/matrix-oauth-callback";
const TOKEN_ROTATION_INTERVAL_SECONDS: u64 = 30 * 60;

#[derive(Default)]
pub struct AuthState {
    inner: Mutex<AuthRuntimeState>,
}

#[derive(Default)]
struct AuthRuntimeState {
    pending_client: Option<Client>,
    client: Option<Client>,
    session: Option<MatrixSession>,
}

#[derive(Clone)]
struct MatrixSession {
    homeserver_url: String,
    user_id: String,
    device_id: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedMatrixSession {
    homeserver_url: String,
    matrix_session: SdkMatrixSession,
}

impl AuthState {
    pub fn client(&self) -> Result<Client, String> {
        let state = self
            .inner
            .lock()
            .map_err(|_| String::from("Failed to acquire auth state lock"))?;

        state
            .client
            .clone()
            .ok_or_else(|| String::from("No authenticated Matrix session"))
    }

    pub async fn restore_client_from_disk_if_needed(&self, app: &AppHandle) -> Result<(), String> {
        {
            let state = self
                .inner
                .lock()
                .map_err(|_| String::from("Failed to acquire auth state lock"))?;

            if state.client.is_some() {
                return Ok(());
            }
        }

        let persisted = load_persisted_session(app)?;
        let Some(persisted) = persisted else {
            return Ok(());
        };

        let client = Client::builder()
            .server_name_or_homeserver_url(persisted.homeserver_url.clone())
            .handle_refresh_tokens()
            .build()
            .await
            .map_err(|error| format!("Failed to initialize Matrix client: {error}"))?;

        client
            .matrix_auth()
            .restore_session(persisted.matrix_session.clone(), RoomLoadSettings::default())
            .await
            .map_err(|error| format!("Failed to restore Matrix session: {error}"))?;

        let mut state = self
            .inner
            .lock()
            .map_err(|_| String::from("Failed to acquire auth state lock"))?;

        state.client = Some(client);
        state.session = Some(MatrixSession {
            homeserver_url: persisted.homeserver_url,
            user_id: persisted.matrix_session.meta.user_id.to_string(),
            device_id: persisted.matrix_session.meta.device_id.to_string(),
        });

        Ok(())
    }
}

pub fn start_token_rotation_worker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(
            TOKEN_ROTATION_INTERVAL_SECONDS,
        ));

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixStartOAuthRequest {
    pub homeserver_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixStartOAuthResponse {
    pub authorization_url: String,
    pub redirect_uri: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCompleteOAuthRequest {
    pub callback_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSessionStatusResponse {
    pub authenticated: bool,
    pub homeserver_url: Option<String>,
    pub user_id: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixLogoutResponse {
    pub logged_out: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCompleteOAuthResponse {
    pub authenticated: bool,
    pub homeserver_url: String,
    pub user_id: String,
    pub device_id: String,
}

#[tauri::command]
pub async fn matrix_start_oauth(
    request: MatrixStartOAuthRequest,
    auth_state: State<'_, AuthState>,
) -> Result<MatrixStartOAuthResponse, String> {
    let homeserver_url = normalize_homeserver_url(&request.homeserver_url)?;
    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver_url.clone())
        .handle_refresh_tokens()
        .build()
        .await
        .map_err(|error| format!("Failed to initialize Matrix client: {error}"))?;

    let authorization_url = client
        .matrix_auth()
        .get_sso_login_url(CALLBACK_REDIRECT_URI, None)
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
        redirect_uri: String::from(CALLBACK_REDIRECT_URI),
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
        .login_with_sso_callback(callback_url.into())
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
        &PersistedMatrixSession {
            homeserver_url: homeserver_url.clone(),
            matrix_session: persisted_matrix_session,
        },
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
    auth_state.restore_client_from_disk_if_needed(&app_handle).await?;

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

fn persisted_session_path(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = storage::app_data_dir(app)?;
    Ok(data_dir.join("matrix-session.json"))
}

fn load_persisted_session(app: &AppHandle) -> Result<Option<PersistedMatrixSession>, String> {
    let path = persisted_session_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read persisted Matrix session: {error}"))?;

    let parsed = serde_json::from_str::<PersistedMatrixSession>(&raw)
        .map_err(|error| format!("Failed to parse persisted Matrix session: {error}"))?;

    Ok(Some(parsed))
}

fn persist_session(app: &AppHandle, session: &PersistedMatrixSession) -> Result<(), String> {
    let path = persisted_session_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create app data directory: {error}"))?;
    }

    let raw = serde_json::to_string(session)
        .map_err(|error| format!("Failed to serialize Matrix session: {error}"))?;

    fs::write(path, raw).map_err(|error| format!("Failed to persist Matrix session: {error}"))?;
    Ok(())
}

fn persist_session_from_client(app: &AppHandle, client: &Client) -> Result<(), String> {
    let session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| String::from("Missing Matrix session while persisting token refresh"))?;

    persist_session(
        app,
        &PersistedMatrixSession {
            homeserver_url: client.homeserver().to_string(),
            matrix_session: session,
        },
    )
}

fn clear_persisted_session(app: &AppHandle) -> Result<(), String> {
    let path = persisted_session_path(app)?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("Failed to clear persisted Matrix session: {error}"))?;
    }

    Ok(())
}

fn normalize_homeserver_url(raw: &str) -> Result<String, String> {
    let candidate = raw.trim();
    if candidate.is_empty() {
        return Err(String::from("Homeserver URL is required"));
    }

    let with_scheme = if candidate.contains("://") {
        candidate.to_owned()
    } else {
        format!("https://{}", candidate)
    };

    let parsed = Url::parse(&with_scheme)
        .map_err(|_| String::from("Homeserver URL is not a valid URL"))?;

    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(String::from(
            "Homeserver URL must use http or https scheme",
        ));
    }

    if parsed.host_str().is_none() {
        return Err(String::from("Homeserver URL must include a hostname"));
    }

    let mut normalized = parsed;
    normalized.set_path("");
    normalized.set_query(None);
    normalized.set_fragment(None);

    Ok(normalized.to_string().trim_end_matches('/').to_owned())
}
