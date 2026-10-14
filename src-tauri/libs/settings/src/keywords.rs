//! User-level notification keywords: the keywords matched by the
//! "mentions and keywords only" room notification mode.

use std::sync::Arc;

use auth::state::AuthState;
use storage::AppDb;
use types::settings::{
    MatrixAddKeywordRequest, MatrixAddKeywordResponse, MatrixKeywordsResponse,
    MatrixRemoveKeywordRequest, MatrixRemoveKeywordResponse,
};
use types::Paths;

use crate::commands::require_client;

fn validate_keyword(keyword: &str) -> Result<(), String> {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return Err("Keyword cannot be empty".to_string());
    }
    if trimmed.len() > 64 {
        return Err("Keyword is too long (maximum 64 characters)".to_string());
    }
    Ok(())
}

async fn enabled_keywords(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<Vec<String>, String> {
    let client = require_client(paths, app_db, auth_state).await?;
    let keywords = client
        .notification_settings()
        .await
        .enabled_keywords()
        .await;
    Ok(keywords.into_iter().collect())
}

pub async fn get_keywords(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
) -> Result<MatrixKeywordsResponse, String> {
    Ok(MatrixKeywordsResponse {
        keywords: enabled_keywords(paths, app_db, auth_state).await?,
    })
}

pub async fn add_keyword(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixAddKeywordRequest,
) -> Result<MatrixAddKeywordResponse, String> {
    validate_keyword(&request.keyword)?;

    let client = require_client(paths, app_db, auth_state).await?;
    client
        .notification_settings()
        .await
        .add_keyword(request.keyword.trim().to_owned())
        .await
        .map_err(|error| format!("Failed to add keyword: {error}"))?;

    Ok(MatrixAddKeywordResponse {
        keywords: enabled_keywords(paths, app_db, auth_state).await?,
    })
}

pub async fn remove_keyword(
    paths: &Paths,
    app_db: &Arc<AppDb>,
    auth_state: &AuthState,
    request: &MatrixRemoveKeywordRequest,
) -> Result<MatrixRemoveKeywordResponse, String> {
    validate_keyword(&request.keyword)?;

    let client = require_client(paths, app_db, auth_state).await?;
    client
        .notification_settings()
        .await
        .remove_keyword(request.keyword.trim())
        .await
        .map_err(|error| format!("Failed to remove keyword: {error}"))?;

    Ok(MatrixRemoveKeywordResponse {
        keywords: enabled_keywords(paths, app_db, auth_state).await?,
    })
}
