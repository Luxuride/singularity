use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use keychain::{SecretStore, SystemSecretStore};

static SECRET: OnceLock<String> = OnceLock::new();

pub fn get_secret() -> Option<&'static str> {
    SECRET.get().map(String::as_str)
}

/// Resolve the app database secret from the OS keychain (falling back to a
/// file in `secret_dir`), and cache it in the process-wide [`SECRET`] slot.
/// Tauri-free: the binder passes the resolved data directory.
pub async fn init_secret(
    secret_dir: &Path,
    service_name: &str,
    account_name: &str,
    bytes_len: usize,
) -> Result<(), String> {
    let secret = get_or_create_secret(
        &SystemSecretStore,
        secret_dir,
        service_name,
        account_name,
        bytes_len,
    )
    .await?;
    // Idempotent: the secret store may have already set SECRET internally.
    SECRET.get_or_init(|| secret);
    Ok(())
}

/// Path of the fallback secret file for `account_name` inside `secret_dir`.
fn fallback_secret_path(secret_dir: &Path, account_name: &str) -> PathBuf {
    secret_dir.join(format!("{account_name}.secret"))
}

/// Resolve the app database secret.
///
/// The keychain and the fallback file are two copies of one value, so they are
/// reconciled rather than raced: whichever store already holds a secret is
/// authoritative, the other is either re-seeded from it or removed. Minting a
/// *new* secret is the last resort, because a fresh secret cannot open a
/// database written under the previous one — that silently loses the user's
/// message history.
///
/// The resolved secret is also cached in the process-wide [`SECRET`] slot so
/// that later callers (e.g. the encrypted database) can read it without
/// re-querying the keychain.
pub async fn get_or_create_secret<S: SecretStore>(
    store: &S,
    secret_dir: &Path,
    service_name: &str,
    account_name: &str,
    bytes_len: usize,
) -> Result<String, String> {
    let path = fallback_secret_path(secret_dir, account_name);

    let keychain_secret = match store.get(service_name, account_name).await {
        Ok(Some(secret)) if !secret.trim().is_empty() => Some(secret),
        // Missing or empty keychain secret: not authoritative.
        Ok(_) => None,
        Err(error) => {
            // A keychain read error is NOT proof that the keychain is empty: a
            // locked keyring or a not-yet-ready Secret Service fails here while
            // the entry still exists. Never mint a new secret in response.
            log::warn!("Keychain read failed, consulting fallback file secret storage: {error}");
            None
        }
    };

    if let Some(secret) = keychain_secret {
        if let Some(file_secret) = read_file_secret(&path)? {
            if file_secret != secret {
                log::error!(
                    "Keychain and fallback file hold DIFFERENT database secrets; the keychain \
                     copy wins and the file is removed. If this app profile has a database you \
                     still need, restore the original keychain entry from a backup."
                );
            }
            remove_file_secret(&path);
        }
        SECRET.set(secret.clone()).ok();
        return Ok(secret);
    }

    // The keychain is unusable or empty, so an existing file is the only
    // remaining copy of the real secret. Re-seed the keychain with it so the
    // two stores converge instead of diverging on every launch.
    if let Some(secret) = read_file_secret(&path)? {
        if let Err(error) = store.set(service_name, account_name, &secret).await {
            log::warn!("Failed to re-seed keychain from fallback file secret: {error}");
        }
        SECRET.set(secret.clone()).ok();
        return Ok(secret);
    }

    // Neither store holds a secret: this really is a first run.
    let encoded = generate_secret(bytes_len);

    if let Err(error) = store.set(service_name, account_name, &encoded).await {
        log::warn!("Failed to store keychain secret: {error}");
    }

    // Persist to the file as well: if the keychain write above failed, the
    // file is the only copy left once this process exits.
    write_file_secret(&path, &encoded)?;

    SECRET.set(encoded.clone()).ok();
    Ok(encoded)
}

fn generate_secret(bytes_len: usize) -> String {
    let mut secret_bytes = vec![0_u8; bytes_len.max(32)];
    let mut rng = rand::rngs::OsRng;
    rand::RngCore::fill_bytes(&mut rng, &mut secret_bytes);
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, secret_bytes)
}

/// Read the fallback secret file.
///
/// Returns `Ok(None)` only when the file genuinely does not exist. An existing
/// file that is empty or non-UTF-8 is corruption, not a first run: overwriting
/// it would replace the only surviving copy of a key the database depends on,
/// so that is reported as an error instead.
fn read_file_secret(path: &Path) -> Result<Option<String>, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create secret storage directory: {error}"))?;
    }

    match std::fs::read_to_string(path) {
        Ok(secret) => {
            if secret.trim().is_empty() {
                return Err(format!(
                    "Fallback app database secret at {path:?} is empty. Refusing to overwrite it: \
                     regenerating would make the encrypted database unreadable. Restore the file \
                     from a backup, or delete it to start over with a new database."
                ));
            }
            Ok(Some(secret))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "Failed to read fallback app database secret at {path:?}: {error}"
        )),
    }
}

/// Write the fallback secret file with owner-only permissions.
///
/// Written to a sibling temp file and renamed, so a crash or a full disk can
/// never leave a truncated secret file behind — the previous file survives
/// intact until the rename replaces it atomically.
fn write_file_secret(path: &Path, secret: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Secret path {path:?} has no parent directory"))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create secret storage directory: {error}"))?;

    let nonce = rand::random::<u64>();
    let temp_path = parent.join(format!(".secret-{}-{nonce}.tmp", std::process::id()));

    let write_result = (|| -> std::io::Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Set the mode at creation so the plaintext key is never briefly
            // world-readable, and the rename below preserves it.
            options.mode(0o600);
        }
        let mut file = options.open(&temp_path)?;
        std::io::Write::write_all(&mut file, secret.as_bytes())?;
        file.sync_all()
    })();

    if let Err(error) = write_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Failed to persist fallback app database secret: {error}"
        ));
    }

    #[cfg(unix)]
    {
        // Re-assert on the reuse path too: a file created by an older build, or
        // one whose mode was widened by a backup/restore tool, must not stay
        // readable by other local users.
        if let Err(error) =
            std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o600))
        {
            let _ = std::fs::remove_file(&temp_path);
            return Err(format!(
                "Failed to set fallback secret file permissions: {error}"
            ));
        }
    }

    std::fs::rename(&temp_path, path).map_err(|error| {
        let _ = std::fs::remove_file(&temp_path);
        format!("Failed to move fallback app database secret into place: {error}")
    })
}

fn remove_file_secret(path: &Path) {
    if let Err(error) = std::fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!(
                "Failed to remove redundant fallback secret at {path:?}: {error}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// In-memory `SecretStore` for exercising the get-or-create logic without a
    /// real keychain.
    #[derive(Default)]
    struct MockStore {
        entries: Mutex<std::collections::HashMap<String, String>>,
    }

    impl SecretStore for MockStore {
        async fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
            let key = format!("{service}:{account}");
            Ok(self.entries.lock().unwrap().get(&key).cloned())
        }

        async fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
            let key = format!("{service}:{account}");
            self.entries.lock().unwrap().insert(key, value.to_string());
            Ok(())
        }

        async fn delete(&self, service: &str, account: &str) -> Result<(), String> {
            let key = format!("{service}:{account}");
            self.entries.lock().unwrap().remove(&key);
            Ok(())
        }
    }

    /// A `SecretStore` whose operations always fail, forcing the file fallback.
    struct FailingStore;

    impl SecretStore for FailingStore {
        async fn get(&self, _service: &str, _account: &str) -> Result<Option<String>, String> {
            Err(String::from("keychain unavailable"))
        }

        async fn set(&self, _service: &str, _account: &str, _value: &str) -> Result<(), String> {
            Err(String::from("keychain unavailable"))
        }

        async fn delete(&self, _service: &str, _account: &str) -> Result<(), String> {
            Err(String::from("keychain unavailable"))
        }
    }

    fn temp_secret_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "singularity-secret-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn creates_secret_when_keychain_is_empty() {
        let store = MockStore::default();
        let dir = temp_secret_dir();

        let secret = get_or_create_secret(&store, &dir, "svc", "acct", 32)
            .await
            .unwrap();

        assert!(!secret.trim().is_empty());
        assert!(store.get("svc", "acct").await.unwrap().is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn reuses_existing_keychain_secret() {
        let store = MockStore::default();
        let dir = temp_secret_dir();
        store.set("svc", "acct", "existing-secret").await.unwrap();

        let secret = get_or_create_secret(&store, &dir, "svc", "acct", 32)
            .await
            .unwrap();

        assert_eq!(secret, "existing-secret");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn falls_back_to_file_when_keychain_read_fails() {
        let store = FailingStore;
        let dir = temp_secret_dir();

        let secret = get_or_create_secret(&store, &dir, "svc", "acct", 32)
            .await
            .unwrap();

        assert!(!secret.trim().is_empty());
        let file = dir.join("acct.secret");
        assert!(file.exists());
        assert_eq!(
            std::fs::read_to_string(&file).unwrap().trim(),
            secret.trim()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression: a transient keychain failure must not mint a *second* secret.
    /// The file secret is the only surviving copy, so it has to win.
    #[tokio::test]
    async fn keychain_failure_reuses_existing_file_secret() {
        let dir = temp_secret_dir();
        let path = dir.join("acct.secret");
        write_file_secret(&path, "file-secret").unwrap();

        // Keychain unreachable, but the file already holds the real secret.
        let secret = get_or_create_secret(&FailingStore, &dir, "svc", "acct", 32)
            .await
            .unwrap();

        assert_eq!(secret, "file-secret");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "file-secret");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression: the same secret must survive a keychain outage either way
    /// round, so the two stores converge instead of diverging per launch.
    #[tokio::test]
    async fn keychain_is_reseeded_from_file_after_an_outage() {
        let dir = temp_secret_dir();
        let path = dir.join("acct.secret");
        write_file_secret(&path, "file-secret").unwrap();

        let store = MockStore::default();
        let secret = get_or_create_secret(&store, &dir, "svc", "acct", 32)
            .await
            .unwrap();

        assert_eq!(secret, "file-secret");
        assert_eq!(store.get("svc", "acct").await.unwrap().as_deref(), Some("file-secret"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression: with a keychain secret present, a stale file from an earlier
    /// outage must not shadow it.
    #[tokio::test]
    async fn keychain_secret_wins_over_a_divergent_file() {
        let store = MockStore::default();
        store.set("svc", "acct", "keychain-secret").await.unwrap();
        let dir = temp_secret_dir();
        let path = dir.join("acct.secret");
        write_file_secret(&path, "divergent-file-secret").unwrap();

        let secret = get_or_create_secret(&store, &dir, "svc", "acct", 32)
            .await
            .unwrap();

        assert_eq!(secret, "keychain-secret");
        assert!(!path.exists(), "divergent file secret should be removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression: an empty or unreadable file must be reported, never
    /// overwritten, because regenerating bricks the encrypted database.
    #[tokio::test]
    async fn refuses_to_overwrite_a_corrupt_secret_file() {
        let dir = temp_secret_dir();
        let path = dir.join("acct.secret");
        std::fs::write(&path, "").unwrap();

        let error = get_or_create_secret(&FailingStore, &dir, "svc", "acct", 32)
            .await
            .expect_err("an empty secret file must not be silently regenerated");

        assert!(error.contains("empty"), "unexpected error: {error}");
        assert!(path.exists(), "the corrupt file must be left in place");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fallback_secret_file_is_owner_only() {
        let dir = temp_secret_dir();
        let path = dir.join("acct.secret");
        write_file_secret(&path, "file-secret").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "fallback secret file must not be group/other readable");

        // The reuse path must re-assert the mode, not just the create path.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_file_secret(&path, "file-secret-2").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn read_file_secret_treats_missing_as_none() {
        let dir = temp_secret_dir();
        let path = dir.join("absent.secret");
        assert!(read_file_secret(&path).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn write_file_secret_replaces_atomically_without_leaving_temp_files() {
        let dir = temp_secret_dir();
        let path = dir.join("acct.secret");
        write_file_secret(&path, "first").unwrap();
        write_file_secret(&path, "second").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        let leftovers: Vec<_> = std::fs::read_dir(Path::new(&dir))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name != "acct.secret")
            .collect();
        assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
