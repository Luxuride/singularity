pub(crate) mod commands;
mod persistence;
mod workers;

use matrix_sdk::store::RoomLoadSettings;
use matrix_sdk::Client;
use std::sync::Mutex;
use tauri::AppHandle;

pub use workers::start_token_rotation_worker;

mod stateful_types {
    use serde::{Deserialize, Serialize};

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
}

#[derive(Default)]
pub struct AuthState {
    pub(crate) inner: Mutex<AuthRuntimeState>,
}

#[derive(Default)]
pub(crate) struct AuthRuntimeState {
    pub(crate) pending_client: Option<Client>,
    pub(crate) client: Option<Client>,
    pub(crate) session: Option<MatrixSession>,
}

#[derive(Clone)]
pub(crate) struct MatrixSession {
    pub(crate) homeserver_url: String,
    pub(crate) user_id: String,
    pub(crate) device_id: String,
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

        let persisted = persistence::load_persisted_session(app)?;
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
            .restore_session(
                persisted.matrix_session.clone(),
                RoomLoadSettings::default(),
            )
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
