// Tauri event names emitted by the Rust backend. Mirrors
// src-tauri/libs/types/src/event_paths.rs.
export const EVENT_ROOM_ADDED = "matrix://rooms/added";
export const EVENT_ROOM_UPDATED = "matrix://rooms/updated";
export const EVENT_ROOM_REMOVED = "matrix://rooms/removed";
export const EVENT_SELECTED_ROOM_MESSAGES = "matrix://rooms/selected/messages";
export const EVENT_CHAT_MESSAGES_STREAM = "matrix://rooms/messages/stream";
export const EVENT_VERIFICATION_STATE_CHANGED = "matrix://verification/state";
export const EVENT_MEDIA_TRANSCODE_PROGRESS = "matrix://media/transcode/progress";