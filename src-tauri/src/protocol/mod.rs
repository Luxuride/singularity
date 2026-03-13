pub mod config {
    pub const CALLBACK_REDIRECT_URI: &str = "http://127.0.0.1:8743/matrix-oauth-callback";
    pub const TOKEN_ROTATION_INTERVAL_SECONDS: u64 = 30 * 60;
    pub const SYNC_TIMEOUT_SECONDS: u64 = 5;
    pub const ROOM_UPDATE_POLL_INTERVAL_SECONDS: u64 = 30;
}

pub mod event_paths {
    pub const ROOM_ADDED: &str = "matrix://rooms/added";
    pub const ROOM_UPDATED: &str = "matrix://rooms/updated";
    pub const ROOM_REMOVED: &str = "matrix://rooms/removed";
    pub const SELECTED_ROOM_MESSAGES: &str = "matrix://rooms/selected/messages";
}

pub mod event_types {
    pub const ROOM_MESSAGE: &str = "m.room.message";
    pub const ROOM_ENCRYPTED: &str = "m.room.encrypted";

    pub mod message_types {
        pub const TEXT: &str = "m.text";
        pub const NOTICE: &str = "m.notice";
        pub const EMOTE: &str = "m.emote";
    }
}

pub mod storage_keys {
    pub const CHATS_CACHE_FILE: &str = "matrix-chats-cache.json";
    pub const SESSION_FILE: &str = "matrix-session.json";
}
