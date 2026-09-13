use matrix_sdk::authentication::matrix::MatrixSession as SdkMatrixSession;
use matrix_sdk::Client;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use storage::AppDb;
use types::{storage_keys, Paths};

pub struct PersistedMatrixSession {
    pub homeserver_url: String,
    pub matrix_session: SdkMatrixSession,
}

impl PersistedMatrixSession {
    pub fn new(homeserver_url: String, matrix_session: SdkMatrixSession) -> Self {
        Self {
            homeserver_url,
            matrix_session,
        }
    }
}

pub fn matrix_sdk_store_path(paths: &Paths) -> PathBuf {
    paths.data_file(storage_keys::MATRIX_SDK_STORE_DIR)
}

pub fn prepare_matrix_sdk_store(paths: &Paths) -> Result<PathBuf, String> {
    let store_path = matrix_sdk_store_path(paths);
    fs::create_dir_all(&store_path)
        .map_err(|error| format!("Failed to create Matrix SDK store directory: {error}"))?;
    Ok(store_path)
}

pub fn load_persisted_session(
    app_db: &Arc<AppDb>,
) -> Result<Option<PersistedMatrixSession>, String> {
    let loaded = app_db.load_persisted_session()?;
    Ok(
        loaded.map(|(homeserver_url, matrix_session)| PersistedMatrixSession {
            homeserver_url,
            matrix_session,
        }),
    )
}

pub fn persist_session(
    app_db: &Arc<AppDb>,
    session: &PersistedMatrixSession,
) -> Result<(), String> {
    app_db.persist_session(&session.homeserver_url, &session.matrix_session)
}

pub fn persist_session_from_client(app_db: &Arc<AppDb>, client: &Client) -> Result<(), String> {
    let session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| String::from("Missing Matrix session while persisting token refresh"))?;

    persist_session(
        app_db,
        &PersistedMatrixSession::new(client.homeserver().to_string(), session),
    )
}

pub fn clear_persisted_session(app_db: &Arc<AppDb>) -> Result<(), String> {
    app_db.clear_session()
}

pub fn clear_app_cache(app_db: &Arc<AppDb>) -> Result<(), String> {
    app_db.clear_app_cache()
}

pub fn clear_app_cache_except_auth(app_db: &Arc<AppDb>) -> Result<(), String> {
    app_db.clear_non_auth_cache()
}

pub fn clear_matrix_sdk_store(paths: &Paths) -> Result<(), String> {
    let path = matrix_sdk_store_path(paths);
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Failed to clear Matrix SDK store: {error}"))?;
    }

    Ok(())
}
