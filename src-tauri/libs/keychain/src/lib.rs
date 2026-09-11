//! A singular, cross-platform interface for reading and writing secrets in the
//! OS-backed credential store.
//!
//! The concrete [`SystemSecretStore`] dispatches to the platform-appropriate
//! backend at compile time:
//! - Windows Credential Manager and the macOS Keychain via the `keyring` crate.
//! - The Linux Secret Service (D-Bus / Flatpak portal) via `oo7`.
//!
//! All fallible operations return `Result<T, String>` to match the rest of the
//! codebase. A missing secret is reported as `Ok(None)`, never as an error.

// This crate is internal to the app (not a published API), so `async fn` in the
// public trait is fine and we don't need to specify auto-trait bounds.
#![allow(async_fn_in_trait)]

#[cfg(any(windows, target_os = "macos"))]
use keyring::Entry;

#[cfg(target_os = "linux")]
mod linux;

/// A store of named secrets keyed by a `(service, account)` pair.
///
/// Implementations must treat a missing secret as `Ok(None)` rather than an
/// error, and must not error when deleting a secret that does not exist.
pub trait SecretStore {
    /// Read the secret for `(service, account)`, or `None` if it does not exist.
    async fn get(&self, service: &str, account: &str) -> Result<Option<String>, String>;

    /// Write `value` as the secret for `(service, account)`, replacing any existing value.
    async fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String>;

    /// Delete the secret for `(service, account)`. A missing secret is not an error.
    ///
    /// Part of the interface contract; not yet invoked by any app flow.
    async fn delete(&self, service: &str, account: &str) -> Result<(), String>;
}

/// The singular secret interface for the running operating system.
///
/// Zero-sized: it carries no state and dispatches to the platform backend.
#[derive(Default)]
pub struct SystemSecretStore;

impl SecretStore for SystemSecretStore {
    async fn get(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            keyring_get(service, account)
        }
        #[cfg(target_os = "linux")]
        {
            linux::get(service, account).await
        }
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            let _ = (service, account);
            Err(String::from(
                "No system secret store is available on this platform",
            ))
        }
    }

    async fn set(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            keyring_set(service, account, value)
        }
        #[cfg(target_os = "linux")]
        {
            linux::set(service, account, value).await
        }
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            let _ = (service, account, value);
            Err(String::from(
                "No system secret store is available on this platform",
            ))
        }
    }

    async fn delete(&self, service: &str, account: &str) -> Result<(), String> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            keyring_delete(service, account)
        }
        #[cfg(target_os = "linux")]
        {
            linux::delete(service, account).await
        }
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        {
            let _ = (service, account);
            Err(String::from(
                "No system secret store is available on this platform",
            ))
        }
    }
}

/// Read a secret from the `keyring`-backed store (Windows / macOS).
#[cfg(any(windows, target_os = "macos"))]
fn keyring_get(service: &str, account: &str) -> Result<Option<String>, String> {
    let entry = Entry::new(service, account)
        .map_err(|error| format!("Failed to open keyring entry: {error}"))?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Failed to read keyring secret: {error}")),
    }
}

/// Write a secret to the `keyring`-backed store (Windows / macOS).
#[cfg(any(windows, target_os = "macos"))]
fn keyring_set(service: &str, account: &str, value: &str) -> Result<(), String> {
    let entry = Entry::new(service, account)
        .map_err(|error| format!("Failed to open keyring entry: {error}"))?;
    entry
        .set_password(value)
        .map_err(|error| format!("Failed to write keyring secret: {error}"))
}

/// Delete a secret from the `keyring`-backed store (Windows / macOS).
#[cfg(any(windows, target_os = "macos"))]
fn keyring_delete(service: &str, account: &str) -> Result<(), String> {
    let entry = Entry::new(service, account)
        .map_err(|error| format!("Failed to open keyring entry: {error}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("Failed to delete keyring secret: {error}")),
    }
}
