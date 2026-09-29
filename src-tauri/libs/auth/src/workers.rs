use matrix_sdk::SessionChange;
use std::sync::Arc;

use storage::AppDb;
use types::Paths;

use crate::persistence::{
    clear_app_cache, clear_matrix_sdk_store, clear_persisted_session, persist_session_from_client,
};
use crate::state::{AuthState, ClientCancelled};

/// Resolve once the session is cleared. `wait_for` returns `Err` when the
/// sender is gone, which only happens if auth state itself is being torn down;
/// treating that as "stop" is the safe reading, so a dropped sender is not a
/// path to a permanently pinned task. Borrowing keeps the `select!` loopable.
async fn wait_for_cancel(cancelled: &mut ClientCancelled) {
    let _ = cancelled.wait_for(|stop| *stop).await;
}

/// Called when a request returns M_UNKNOWN_TOKEN. With `handle_refresh_tokens()` set on the
/// client, the SDK already attempted a silent refresh before surfacing this error, so the
/// token is unrecoverable. Clear the local session and return `Ok(false)` so the caller can
/// transition to a signed-out state gracefully.
pub async fn handle_unknown_token_error(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    _client: &matrix_sdk::Client,
) -> Result<bool, String> {
    log::warn!("Matrix request returned unknown token after automatic refresh; clearing session");
    // Clear before deleting the store directory: `clear_runtime_session` signals
    // the per-client watchers, but they are spawned tasks holding `Client`
    // clones, and the sqlite connection those clones keep open is the reason
    // `remove_dir_all` below is safe at all. Unlinking the directory out from
    // under a live connection leaves the next client on a different inode and
    // the old one still writing to the unlinked files.
    auth_state.clear_runtime_session()?;
    clear_persisted_session(app_db)?;
    clear_app_cache(app_db)?;
    clear_matrix_sdk_store(paths)?;
    Ok(false)
}

/// Watch the SDK session and persist refreshed tokens to the database.
///
/// Takes a [`ClientCancelled`] receiver: the task holds a `Client` clone, and
/// that clone is the last strong reference to the SDK's session-change sender
/// once the session is cleared, so `Closed` never arrives on its own. Without
/// the explicit signal every logout and every unknown-token recovery left a
/// task — and its client — pinned for the life of the process.
pub fn start_session_persistence_watcher(
    app_db: Arc<AppDb>,
    client: matrix_sdk::Client,
    cancelled: ClientCancelled,
) {
    tokio::spawn(async move {
        let mut session_changes = client.subscribe_to_session_changes();
        let mut cancelled = cancelled;

        loop {
            tokio::select! {
                () = wait_for_cancel(&mut cancelled) => break,
                change = session_changes.recv() => match change {
                    Ok(SessionChange::TokensRefreshed) => {
                        if let Err(error) = persist_session_from_client(&app_db, &client) {
                            log::warn!("Failed to persist Matrix session after token refresh: {error}");
                        }
                    }
                    Ok(SessionChange::UnknownToken { .. }) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                },
            }
        }

        log::debug!("Session persistence watcher stopped");
    });
}
