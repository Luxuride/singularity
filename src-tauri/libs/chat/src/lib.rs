//! Chat domain (formerly the `messages` module): message send/receive, media
//! handling, reactions, and emoji packs. Tauri-free: command adapters live in
//! the binder, and domain functions take `&Paths` / `&Arc<AppDb>` /
//! `&AuthState` / `Arc<dyn EventSink>` instead of Tauri state.

pub mod commands;
pub mod emoji;
pub mod helpers;
pub mod media;
pub mod persistence;
pub mod reactions;
pub mod receive;
pub mod send;

pub use commands::{
    cancel_media_transcode, copy_image_to_clipboard, get_chat_messages, get_emoji_packs,
    get_user_avatar, read_clipboard_text, resolve_video_url, send_chat_message, send_media_file,
    stream_chat_messages, toggle_reaction,
};
pub use media::cache_mxc_media_to_local_path;
pub use persistence::store_initial_room_messages;
pub use receive::fetch_room_messages_from_client;
pub use send::MediaTranscodeCancellationState;
