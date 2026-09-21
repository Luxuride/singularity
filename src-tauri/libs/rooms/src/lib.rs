//! Rooms domain: room summaries, navigation, and the background room-update
//! worker that emits room events to the frontend.
//!
//! Tauri-free: command core functions take `&Paths` + `Arc<AppDb>` +
//! `Arc<AuthState>` + `Arc<dyn EventSink>` instead of an `AppHandle`. The
//! room-update worker takes an `Arc<dyn EventSink>` so it can emit events
//! without depending on Tauri.

pub mod commands;
pub mod direct;
pub mod image;
pub mod join;
pub mod navigation;
pub mod persistence;
pub mod workers;

pub use workers::{
    collect_chat_summaries, start_room_update_worker, RoomSnapshot, RoomUpdateWorker,
};
