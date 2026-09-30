use std::sync::OnceLock;

use matrix_sdk::config::SyncSettings;
use matrix_sdk::Client;
use tokio::sync::Mutex;

/// Long enough to be a real sync against a healthy homeserver, short enough
/// that a user waiting on a send does not watch a spinner for the room
/// worker's full long poll.
const BRIEF_SYNC_TIMEOUT_SECONDS: u64 = 2;

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

/// Run a sync that must not outlive a user-visible action.
///
/// `sync_once_default` inherits the long-poll timeout, so a caller that only
/// needs the SDK's store brought up to date can spend the rest of a 25s poll
/// behind the room worker. A short timeout makes the wait bounded instead: a
/// poll that returns nothing new within it is a successful no-op for these
/// callers, and the room worker delivers the rest on its next pass.
pub async fn sync_once_brief(client: &Client) -> Result<(), String> {
    sync_once_serialized(
        client,
        SyncSettings::default().timeout(std::time::Duration::from_secs(BRIEF_SYNC_TIMEOUT_SECONDS)),
    )
    .await
}
