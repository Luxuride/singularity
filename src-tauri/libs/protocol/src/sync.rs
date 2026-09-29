use std::sync::OnceLock;

use matrix_sdk::config::SyncSettings;
use matrix_sdk::Client;
use tokio::sync::Mutex;

/// matrix-sdk serializes `sync_once` internally, so a second caller waits out the
/// first one's full long poll before its own request is even sent. Serializing
/// the sends too bounds a burst of commands (send a message, send a file, toggle
/// a reaction) to one long poll rather than one each.
///
/// The lock is per homeserver, so two accounts in one process do not queue
/// behind each other's sync.
fn sync_lock_for(client: &Client) -> &'static Mutex<()> {
    static LOCKS: OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, &'static Mutex<()>>>,
    > = OnceLock::new();

    let locks = LOCKS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));

    // The registry holds no state that can fail to be usable, so a poison here
    // is an unrelated panic. Recovering keeps sync working rather than taking
    // the session down with it.
    let Ok(mut locks) = locks.lock() else {
        return Box::leak(Box::new(Mutex::new(())));
    };

    locks
        .entry(client.homeserver().to_string())
        .or_insert_with(|| Box::leak(Box::new(Mutex::new(()))))
}

pub async fn sync_once_serialized(client: &Client, settings: SyncSettings) -> Result<(), String> {
    let _guard = sync_lock_for(client).lock().await;
    client
        .sync_once(settings)
        .await
        .map(|_| ())
        .map_err(|error| format!("Failed to sync Matrix client: {error}"))
}

/// Run a single serialized sync with default settings.
pub async fn sync_once_default(client: &Client) -> Result<(), String> {
    sync_once_serialized(client, SyncSettings::default()).await
}
