//! Shared types, DTOs, enums, constants, and cross-cutting abstractions
//! (`Paths`, `EventSink`, room-update trigger types) for the Singularity
//! domain crates. This crate intentionally avoids `matrix-sdk` and `tauri`
//! so it compiles quickly and stays a stable foundation for the other crates.

pub mod auth;
pub mod chat;
pub mod config;
pub mod event_paths;
pub mod event_sink;
pub mod event_types;
pub mod paths;
pub mod rooms;
pub mod settings;
pub mod storage_keys;
pub mod triggers;
pub mod verification;

pub use event_sink::EventSink;
pub use paths::Paths;
pub use triggers::{RoomRefreshTrigger, RoomUpdateTriggerState};
