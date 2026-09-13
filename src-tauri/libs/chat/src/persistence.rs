use std::sync::Arc;

use storage::AppDb;
use types::chat::MatrixGetChatMessagesResponse;

pub fn load_initial_room_messages(
    app_db: &Arc<AppDb>,
    room_id: &str,
    from: Option<&str>,
    limit: Option<u32>,
) -> Result<Option<MatrixGetChatMessagesResponse>, String> {
    if !is_cacheable_initial_request(from, limit) {
        return Ok(None);
    }

    app_db.load_initial_room_messages(room_id)
}

pub fn store_initial_room_messages(
    app_db: &Arc<AppDb>,
    response: &MatrixGetChatMessagesResponse,
) -> Result<(), String> {
    app_db.store_initial_room_messages(response)
}

pub fn is_cacheable_initial_request(from: Option<&str>, limit: Option<u32>) -> bool {
    if from.is_some() {
        return false;
    }

    let effective_limit = limit.unwrap_or(50).min(100);
    effective_limit == 50
}
