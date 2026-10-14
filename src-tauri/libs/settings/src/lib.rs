//! User settings domain: the signed-in user's profile (display name, avatar)
//! and account management (password change). Tauri-free: command adapters live
//! in the binder, and domain functions take `&Paths` + `&Arc<AppDb>` +
//! `&AuthState` instead of Tauri state.

pub mod commands;
pub mod profile;

pub use commands::{change_password, get_profile, remove_avatar, set_avatar, set_display_name};
