//! Settings domain: the signed-in user's profile (display name, avatar) and
//! account management (password change). Tauri-free: command adapters live in
//! the binder, and domain functions take `&Paths` + `&Arc<AppDb>` +
//! `&AuthState` instead of Tauri state.

pub mod commands;
pub mod keywords;
pub mod notification_defaults;
pub mod profile;
pub mod room_keywords;
pub mod room_settings;

pub use commands::{
    add_keyword, add_room_keyword, change_password, get_keywords, get_notification_defaults,
    get_profile, get_room_keywords, remove_avatar, remove_keyword, remove_room_keyword,
    room_get_settings, room_leave, room_remove_tag, room_set_flags, room_set_notification_mode,
    room_set_tag, set_avatar, set_display_name, set_notification_default,
};
