//! Tauri-free persistence layer: OS-keychain-backed secret management and the
//! encrypted application database (`AppDb`). The Tauri binder resolves the
//! data directory (via `types::Paths`) and passes it in, so this crate never
//! depends on `tauri`.

pub mod db;
pub mod secret;

pub use db::AppDb;
