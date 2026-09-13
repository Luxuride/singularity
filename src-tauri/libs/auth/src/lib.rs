//! Authentication: `AuthState` (the shared Matrix client holder), login flows,
//! session persistence, and the session-persistence watcher.
//!
//! Tauri-free: domain functions take `&Paths` + `&Arc<AppDb>` instead of an
//! `AppHandle`. The Tauri binder adapts `#[tauri::command]` wrappers to these
//! core functions and registers the `on_client_ready` hook to start the
//! verification-state watcher without this crate depending on it.

pub mod commands;
pub mod persistence;
pub mod state;
pub mod workers;

pub use commands::{
    clear_cache_except_auth, complete_oauth, logout, password_login, recover_with_key,
    recovery_status, session_status, start_oauth,
};
pub use state::{wait_for_e2ee_initialization, AuthState, MatrixSession};
pub use workers::{handle_unknown_token_error, start_session_persistence_watcher};
