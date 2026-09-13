use matrix_sdk::store::RoomLoadSettings;
use matrix_sdk::Client;
use std::sync::Mutex;

use types::Paths;

use crate::persistence;
use crate::workers::start_session_persistence_watcher;

/// Hook fired whenever a Matrix client becomes ready (restored or logged in).
type ClientReadyHook = Box<dyn Fn(Client) + Send>;

/// Shared Matrix client holder. Tauri-free: `restore_client_from_disk_if_needed`
/// takes `&Paths` + `&Arc<AppDb>` instead of an `AppHandle`.
///
/// The `on_client_ready` hook breaks the auth -> verification cycle: the binder
/// registers a hook that starts the verification-state watcher whenever a client
/// is restored or a login completes, without this crate depending on the
/// verification crate.
#[derive(Default)]
pub struct AuthState {
    inner: Mutex<AuthRuntimeState>,
    on_client_ready: Mutex<Option<ClientReadyHook>>,
}

#[derive(Default)]
struct AuthRuntimeState {
    pending_client: Option<Client>,
    client: Option<Client>,
    session: Option<MatrixSession>,
}

#[derive(Clone)]
pub struct MatrixSession {
    pub homeserver_url: String,
    pub user_id: String,
    pub device_id: String,
}

impl AuthState {
    fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, AuthRuntimeState>, String> {
        self.inner
            .lock()
            .map_err(|_| String::from("Failed to acquire auth state lock"))
    }

    /// Register the hook fired whenever a Matrix client becomes ready (restored
    /// from disk or freshly logged in). The binder uses this to start the
    /// verification-state watcher.
    pub fn set_on_client_ready(&self, hook: ClientReadyHook) {
        let mut guard = match self.on_client_ready.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        *guard = Some(hook);
    }

    /// Fire the `on_client_ready` hook. Called by the login/restore flows once a
    /// Matrix client is ready so the binder can start the verification-state
    /// watcher.
    pub fn fire_client_ready(&self, client: &Client) {
        let guard = match self.on_client_ready.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if let Some(hook) = guard.as_ref() {
            hook(client.clone());
        }
    }

    pub fn client(&self) -> Result<Client, String> {
        self.lock_inner()?
            .client
            .clone()
            .ok_or_else(|| String::from("No authenticated Matrix session"))
    }

    /// Read the current session, if any.
    pub fn session(&self) -> Result<Option<MatrixSession>, String> {
        Ok(self.lock_inner()?.session.clone())
    }

    pub fn clear_runtime_session(&self) -> Result<(), String> {
        let mut state = self.lock_inner()?;

        state.pending_client = None;
        state.session = None;
        state.client = None;

        Ok(())
    }

    /// Store the in-flight OAuth client so `complete_oauth` can finish the login.
    pub fn set_pending_client(&self, client: Client) -> Result<(), String> {
        self.lock_inner()?.pending_client = Some(client);
        Ok(())
    }

    /// Take the in-flight OAuth client, if any.
    pub fn take_pending_client(&self) -> Result<Option<Client>, String> {
        Ok(self.lock_inner()?.pending_client.take())
    }

    /// Record a freshly authenticated client + session.
    pub fn set_authenticated(&self, client: Client, session: MatrixSession) -> Result<(), String> {
        let mut state = self.lock_inner()?;
        state.client = Some(client);
        state.session = Some(session);
        Ok(())
    }

    pub async fn restore_client_from_disk_if_needed(
        &self,
        paths: &Paths,
        app_db: &std::sync::Arc<storage::AppDb>,
    ) -> Result<(), String> {
        {
            if self.lock_inner()?.client.is_some() {
                return Ok(());
            }
        }

        let persisted = persistence::load_persisted_session(app_db)?;
        let Some(persisted) = persisted else {
            return Ok(());
        };

        let store_path = persistence::prepare_matrix_sdk_store(paths)?;

        let client = Client::builder()
            .server_name_or_homeserver_url(persisted.homeserver_url.clone())
            .sqlite_store(&store_path, None)
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

        {
            let mut state = self.lock_inner()?;

            state.client = Some(client.clone());
            state.session = Some(MatrixSession {
                homeserver_url: persisted.homeserver_url,
                user_id: persisted.matrix_session.meta.user_id.to_string(),
                device_id: persisted.matrix_session.meta.device_id.to_string(),
            });
        }

        start_session_persistence_watcher(app_db.clone(), client.clone());
        self.fire_client_ready(&client);

        Ok(())
    }

    pub async fn restore_client_and_get(
        &self,
        paths: &Paths,
        app_db: &std::sync::Arc<storage::AppDb>,
    ) -> Result<Client, String> {
        self.restore_client_from_disk_if_needed(paths, app_db)
            .await?;
        self.client()
    }
}

pub async fn wait_for_e2ee_initialization(client: &Client) {
    client
        .encryption()
        .wait_for_e2ee_initialization_tasks()
        .await;
}
