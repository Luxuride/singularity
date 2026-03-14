use matrix_sdk::authentication::matrix::MatrixSession as SdkMatrixSession;
use matrix_sdk::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

use crate::protocol::storage_keys;
use crate::storage;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersistedMatrixSession {
    #[serde(default = "cache_schema_version")]
    version: u32,
    pub(crate) homeserver_url: String,
    pub(crate) matrix_session: SdkMatrixSession,
}

impl PersistedMatrixSession {
    pub(crate) fn new(homeserver_url: String, matrix_session: SdkMatrixSession) -> Self {
        Self {
            version: cache_schema_version(),
            homeserver_url,
            matrix_session,
        }
    }
}

const fn cache_schema_version() -> u32 {
    storage_keys::CACHE_SCHEMA_VERSION
}

fn persisted_session_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage::app_data_file(app, storage_keys::SESSION_FILE)
}

fn legacy_persisted_session_path(app: &AppHandle) -> Result<PathBuf, String> {
    storage::app_data_file(app, storage_keys::SESSION_FILE_LEGACY)
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
    let path = persisted_session_path(app)?;
    let path = if path.exists() {
        path
    } else {
        let legacy_path = legacy_persisted_session_path(app)?;
        if legacy_path.exists() {
            legacy_path
        } else {
            return Ok(None);
        }
    };

    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read persisted Matrix session: {error}"))?;

    let parsed = serde_json::from_str::<PersistedMatrixSession>(&raw)
        .map_err(|error| format!("Failed to parse persisted Matrix session: {error}"))?;

    Ok(Some(parsed))
}

pub(crate) fn persist_session(
    app: &AppHandle,
    session: &PersistedMatrixSession,
) -> Result<(), String> {
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
    let path = persisted_session_path(app)?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("Failed to clear persisted Matrix session: {error}"))?;
    }

    let legacy_path = legacy_persisted_session_path(app)?;
    if legacy_path.exists() {
        fs::remove_file(legacy_path)
            .map_err(|error| format!("Failed to clear persisted Matrix session: {error}"))?;
    }

    Ok(())
}

pub(crate) fn clear_matrix_sdk_store(app: &AppHandle) -> Result<(), String> {
    let path = matrix_sdk_store_path(app)?;
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Failed to clear Matrix SDK store: {error}"))?;
    }

    Ok(())
}
