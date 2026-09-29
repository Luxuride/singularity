use matrix_sdk::store::RoomLoadSettings;
use matrix_sdk::Client;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use types::Paths;

use crate::persistence;
use crate::workers::start_session_persistence_watcher;

/// Handle a background task uses to learn that its client is no longer the
/// live one. Sending `true` on the sender is what ends the task.
pub type ClientCancelled = watch::Receiver<bool>;

/// Hook fired whenever a Matrix client becomes ready (restored or logged in).
///
/// The hook also receives a [`ClientCancelled`] receiver so the task it starts
/// ends when the session is cleared. Without it, a task holding a `Client`
/// clone keeps the SDK's own broadcast sender alive forever — see
/// `clear_runtime_session`.
type ClientReadyHook = Arc<dyn Fn(Client, ClientCancelled) + Send + Sync>;

/// RAII guard for the restore single-flight flag: any exit path, including a
/// build error, releases it.
struct RestoreGuard<'a>(&'a AtomicBool);

impl Drop for RestoreGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

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
    restore_in_progress: AtomicBool,
}

#[derive(Default)]
struct AuthRuntimeState {
    pending_client: Option<Client>,
    client: Option<Client>,
    session: Option<MatrixSession>,
    deep_link_registered: bool,
    /// Fires the per-client background tasks. Dropped on session clear, which
    /// is what tells them to stop.
    client_cancelled: Option<watch::Sender<bool>>,
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
    ///
    /// Returns a receiver for the same cancellation signal the hook receives,
    /// so a caller starting its own per-client task can share one signal rather
    /// than inventing a second, independent way for the session to be declared
    /// dead.
    pub fn fire_client_ready(&self, client: &Client) -> ClientCancelled {
        // A fresh sender per ready event, so a new client never inherits the
        // previous one's cancellation.
        let (sender, receiver) = watch::channel(false);
        let mut state = match self.inner.lock() {
            Ok(state) => state,
            Err(_) => return receiver_never_fires(),
        };
        state.client_cancelled = Some(sender);
        drop(state);

        // Cloned, not taken: the hook is called outside the lock so a hook that
        // touches auth state cannot deadlock against it.
        let hook = {
            let guard = match self.on_client_ready.lock() {
                Ok(guard) => guard,
                Err(_) => return receiver,
            };
            guard.as_ref().map(Arc::clone)
        };
        if let Some(hook) = hook {
            hook(client.clone(), receiver.clone());
        }

        receiver
    }

    /// Record whether the `singularity://` deep-link scheme is registered with
    /// the OS. The binder sets this from the `register_all()` result during
    /// setup; when registration fails the OAuth flow falls back to manual
    /// callback paste because the browser redirect can't route back to the app.
    pub fn set_deep_link_registered(&self, registered: bool) {
        let mut state = match self.lock_inner() {
            Ok(state) => state,
            Err(_) => return,
        };
        state.deep_link_registered = registered;
    }

    /// Whether the `singularity://` deep-link scheme is registered. Defaults to
    /// `true` (assume registered) if the state lock is unavailable.
    pub fn deep_link_registered(&self) -> bool {
        match self.lock_inner() {
            Ok(state) => state.deep_link_registered,
            Err(_) => true,
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
        // Signal before dropping the sender, so a watcher is already awake by
        // the time the last external `Client` clone goes away. The watchers
        // hold `Client` clones of their own, which means the SDK's broadcast
        // sender never closes on its own and the tasks would otherwise run for
        // the life of the process.
        if let Some(cancelled) = state.client_cancelled.take() {
            let _ = cancelled.send(true);
        }

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

        // The check above cannot be held across the build and `restore_session`,
        // both of which await, and `session_status` / `recovery_status` /
        // `recover_with_key` are independent commands that can be in flight at
        // once on first launch. Two callers both saw `client == None`, both
        // built a `Client` against the same sqlite path, and the second
        // silently overwrote the first — which by then was referenced only by
        // its own watcher task. One caller builds; the rest wait and observe
        // its result.
        loop {
            if self.lock_inner()?.client.is_some() {
                return Ok(());
            }

            if self
                .restore_in_progress
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }

            // Another caller is building. Wait for it rather than starting a
            // second one, and re-check afterwards: the flag is released even if
            // that build failed, in which case this caller takes over.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let _guard = RestoreGuard(&self.restore_in_progress);

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

        let cancelled = self.fire_client_ready(&client);
        start_session_persistence_watcher(app_db.clone(), client.clone(), cancelled);

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

/// A receiver that never fires, for the poisoned-lock case where no signal can
/// be stored. The sender is leaked deliberately: a dropped sender would make
/// `wait_for` return `Err` and stop every task immediately, which is the wrong
/// failure direction for a lock that is merely poisoned.
fn receiver_never_fires() -> ClientCancelled {
    let (sender, receiver) = watch::channel(false);
    Box::leak(Box::new(sender));
    receiver
}

pub async fn wait_for_e2ee_initialization(client: &Client) {
    client
        .encryption()
        .wait_for_e2ee_initialization_tasks()
        .await;
}
