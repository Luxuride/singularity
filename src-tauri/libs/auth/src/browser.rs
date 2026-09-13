//! OS browser launching for the OAuth sign-in flow.
//!
//! Tauri-free: delegates to the `open` crate, which shells out to the
//! platform's default URL handler. Returns an error when a browser cannot be
//! launched (headless container, missing URL handler, etc.), which the OAuth
//! flow uses to fall back to manual URL copy and callback paste.

use open::that;

/// Open `url` in the user's default browser.
///
/// Returns `Ok(())` when a browser process was launched, or an error when the
/// platform URL handler is unavailable. This is the signal the OAuth flow uses
/// to switch to the copy/paste fallback.
///
/// Note: this blocks until the platform opener exits, so callers on an async
/// runtime should run it via `tokio::task::spawn_blocking`.
pub fn open_in_browser(url: &str) -> Result<(), String> {
    that(url).map_err(|error| format!("Failed to open browser: {error}"))
}
