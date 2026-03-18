use matrix_sdk::authentication::matrix::MatrixSession as SdkMatrixSession;
use matrix_sdk::Client;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::db::AppDb;
use crate::protocol::storage_keys;
use crate::storage;

pub(crate) struct PersistedMatrixSession {
    pub(crate) homeserver_url: String,
    pub(crate) matrix_session: SdkMatrixSession,
}

impl PersistedMatrixSession {
    pub(crate) fn new(homeserver_url: String, matrix_session: SdkMatrixSession) -> Self {
        Self {
            homeserver_url,
            matrix_session,
        }
    }
}

pub(crate) fn matrix_sdk_store_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage::app_data_file(app, storage_keys::MATRIX_SDK_STORE_DIR)
}

pub(crate) fn prepare_matrix_sdk_store(app: &AppHandle) -> Result<PathBuf, String> {
    let store_path = matrix_sdk_store_path(app)?;
    fs::create_dir_all(&store_path)
        .map_err(|error| format!("Failed to create Matrix SDK store directory: {error}"))?;
    Ok(store_path)
}

pub(crate) fn load_persisted_session(
    app: &AppHandle,
) -> Result<Option<PersistedMatrixSession>, String> {
    let app_db = app.state::<AppDb>();
    let loaded = app_db.load_persisted_session()?;
    Ok(
        loaded.map(|(homeserver_url, matrix_session)| PersistedMatrixSession {
            homeserver_url,
            matrix_session,
        }),
    )
}

pub(crate) fn persist_session(
    app: &AppHandle,
    session: &PersistedMatrixSession,
) -> Result<(), String> {
    let app_db = app.state::<AppDb>();
    app_db.persist_session(&session.homeserver_url, &session.matrix_session)
}

pub(crate) fn persist_session_from_client(app: &AppHandle, client: &Client) -> Result<(), String> {
    let session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| String::from("Missing Matrix session while persisting token refresh"))?;

    persist_session(
        app,
        &PersistedMatrixSession::new(client.homeserver().to_string(), session),
    )
}

pub(crate) fn clear_persisted_session(app: &AppHandle) -> Result<(), String> {
    let app_db = app.state::<AppDb>();
    app_db.clear_session()
}

pub(crate) fn clear_app_cache(app: &AppHandle) -> Result<(), String> {
    let app_db = app.state::<AppDb>();
    app_db.clear_app_cache()
}

pub(crate) fn clear_app_cache_except_auth(app: &AppHandle) -> Result<(), String> {
    let app_db = app.state::<AppDb>();
    app_db.clear_non_auth_cache()
}

pub(crate) fn clear_matrix_sdk_store(app: &AppHandle) -> Result<(), String> {
    let path = matrix_sdk_store_path(app)?;
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Failed to clear Matrix SDK store: {error}"))?;
    }

    Ok(())
}
