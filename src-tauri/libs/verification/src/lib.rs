//! Device verification: verification command logic and the verification-state
//! watcher that emits state-change events to the frontend.
//!
//! Tauri-free: command core functions take `&Paths` + `&Arc<AppDb>` +
//! `&AuthState` instead of `AppHandle`/`State`. The verification-state watcher
//! takes an `Arc<dyn EventSink>` so it can emit events without depending on
//! Tauri. The binder registers this watcher as the `AuthState::on_client_ready`
//! hook.

pub mod commands;

pub use commands::{
    accept_sas_verification, accept_verification_request, confirm_sas_verification,
    get_user_devices, get_verification_flow, own_verification_status, request_device_verification,
    start_sas_verification, start_verification_state_watcher,
};
