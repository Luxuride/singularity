use matrix_sdk::SessionChange;
use std::sync::Arc;

use storage::AppDb;
use types::Paths;

use crate::persistence::{
    clear_app_cache, clear_matrix_sdk_store, clear_persisted_session, persist_session_from_client,
};
use crate::state::AuthState;

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
    auth_state.clear_runtime_session()?;
    clear_persisted_session(app_db)?;
    clear_app_cache(app_db)?;
    clear_matrix_sdk_store(paths)?;
    Ok(false)
}

pub fn start_session_persistence_watcher(app_db: Arc<AppDb>, client: matrix_sdk::Client) {
    tokio::spawn(async move {
        let mut session_changes = client.subscribe_to_session_changes();

        loop {
            match session_changes.recv().await {
                Ok(SessionChange::TokensRefreshed) => {
                    if let Err(error) = persist_session_from_client(&app_db, &client) {
                        log::warn!("Failed to persist Matrix session after token refresh: {error}");
                    }
                }
                Ok(SessionChange::UnknownToken { .. }) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}
