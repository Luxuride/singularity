pub(crate) mod commands;
mod persistence;
pub(crate) mod types;
mod workers;

use matrix_sdk::store::RoomLoadSettings;
use matrix_sdk::Client;
use std::sync::Mutex;
use tauri::AppHandle;

pub use workers::start_session_persistence_watcher;
pub(crate) use workers::handle_unknown_token_error;

use crate::verification::start_verification_state_watcher;

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
    pub(crate) fn lock_inner(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, AuthRuntimeState>, String> {
        self.inner
            .lock()
            .map_err(|_| String::from("Failed to acquire auth state lock"))
    }

    pub fn client(&self) -> Result<Client, String> {
        self.lock_inner()?
            .client
            .clone()
            .ok_or_else(|| String::from("No authenticated Matrix session"))
    }

    pub fn clear_runtime_session(&self) -> Result<(), String> {
        let mut state = self.lock_inner()?;

        state.pending_client = None;
        state.session = None;
        state.client = None;

        Ok(())
    }

    pub async fn restore_client_from_disk_if_needed(&self, app: &AppHandle) -> Result<(), String> {
        {
            if self.lock_inner()?.client.is_some() {
                return Ok(());
            }
        }

        let persisted = persistence::load_persisted_session(app)?;
        let Some(persisted) = persisted else {
            return Ok(());
        };

        let store_path = persistence::prepare_matrix_sdk_store(app)?;

        let client = Client::builder()
            .server_name_or_homeserver_url(persisted.homeserver_url.clone())
            .sqlite_store(&store_path, None)
            .cross_process_store_locks_holder_name(cross_process_lock_holder_name())
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

        if let Err(error) = client
            .encryption()
            .enable_cross_process_store_lock(
                client.cross_process_store_locks_holder_name().to_owned(),
            )
            .await
        {
            log::warn!("Failed to enable cross-process crypto store lock: {error}");
        }

        {
            let mut state = self.lock_inner()?;

            state.client = Some(client.clone());
            state.session = Some(MatrixSession {
                homeserver_url: persisted.homeserver_url,
                user_id: persisted.matrix_session.meta.user_id.to_string(),
                device_id: persisted.matrix_session.meta.device_id.to_string(),
            });
        }

        wait_for_e2ee_initialization(&client).await;
        start_session_persistence_watcher(app.clone(), client.clone());
        start_verification_state_watcher(app.clone(), client);

        Ok(())
    }

    pub async fn restore_client_and_get(&self, app: &AppHandle) -> Result<Client, String> {
        self.restore_client_from_disk_if_needed(app).await?;
        self.client()
    }
}

pub(crate) async fn wait_for_e2ee_initialization(client: &Client) {
    client
        .encryption()
        .wait_for_e2ee_initialization_tasks()
        .await;
}

pub(crate) fn cross_process_lock_holder_name() -> String {
    format!("singularity-{}", std::process::id())
}
