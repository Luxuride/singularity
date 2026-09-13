use std::sync::OnceLock;

use matrix_sdk::config::SyncSettings;
use matrix_sdk::Client;
use tokio::sync::Mutex;

fn sync_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub async fn sync_once_serialized(client: &Client, settings: SyncSettings) -> Result<(), String> {
    let _guard = sync_lock().lock().await;
    client
        .sync_once(settings)
        .await
        .map(|_| ())
        .map_err(|error| format!("Failed to sync Matrix client: {error}"))
}
