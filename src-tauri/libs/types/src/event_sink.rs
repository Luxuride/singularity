use serde_json::Value;

/// Abstraction over Tauri event emission so domain crates can emit events to
/// the frontend without depending on `tauri`. The binder implements this by
/// wrapping `AppHandle::emit`. Payloads are passed as `serde_json::Value` so
/// the trait stays object-safe (`dyn EventSink`).
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: &Value) -> Result<(), String>;
}
