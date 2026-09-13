use std::{path::Path, sync::OnceLock};

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

/// Resolve the app database secret, preferring the OS keychain and falling back
/// to a file in `secret_dir` when the keychain is unavailable.
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
    match store.get(service_name, account_name).await {
        Ok(Some(secret)) if !secret.trim().is_empty() => {
            SECRET.set(secret.clone()).ok();
            return Ok(secret);
        }
        // Missing or empty keychain secret: fall through to generate a new one.
        Ok(_) => {}
        Err(error) => {
            log::warn!("Keychain read failed, falling back to file secret storage: {error}");
            return get_or_create_file_secret(secret_dir, account_name, bytes_len);
        }
    }

    let encoded = generate_secret(bytes_len);

    if let Err(error) = store.set(service_name, account_name, &encoded).await {
        log::warn!("Failed to store keychain secret, falling back to file storage: {error}");
        return get_or_create_file_secret(secret_dir, account_name, bytes_len);
    }

    SECRET.set(encoded.clone()).ok();
    Ok(encoded)
}

fn generate_secret(bytes_len: usize) -> String {
    let mut secret_bytes = vec![0_u8; bytes_len.max(32)];
    let mut rng = rand::rngs::OsRng;
    rand::RngCore::fill_bytes(&mut rng, &mut secret_bytes);
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, secret_bytes)
}

fn get_or_create_file_secret(
    secret_dir: &Path,
    account_name: &str,
    bytes_len: usize,
) -> Result<String, String> {
    let file_name = format!("{account_name}.secret");
    let path = secret_dir.join(file_name);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create secret storage directory: {error}"))?;
    }

    if let Ok(secret) = std::fs::read_to_string(&path) {
        if !secret.trim().is_empty() {
            SECRET.set(secret.clone()).ok();
            return Ok(secret);
        }
    }

    let encoded = generate_secret(bytes_len);

    std::fs::write(&path, &encoded)
        .map_err(|error| format!("Failed to persist fallback app database secret: {error}"))?;

    #[cfg(unix)]
    {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Failed to set fallback secret file permissions: {error}"))?;
    }

    SECRET.set(encoded.clone()).ok();
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
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
}
