-- Schema version 1: the initial application database.
--
-- Everything here is either a cache of server state or a re-derivable
-- session. A migration must never depend on data written by an earlier
-- version of this file; a rebuild from the server is always available.

CREATE TABLE IF NOT EXISTS session_cache (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    homeserver_url TEXT NOT NULL,
    matrix_session BLOB NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chats_cache (
    room_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    image_url TEXT,
    encrypted INTEGER NOT NULL,
    joined_members INTEGER NOT NULL,
    room_kind TEXT NOT NULL DEFAULT 'room',
    joined INTEGER NOT NULL DEFAULT 1,
    is_direct INTEGER NOT NULL DEFAULT 0,
    children_room_ids TEXT NOT NULL DEFAULT '[]',
    position INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS root_space_order (
    room_id TEXT PRIMARY KEY,
    position INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chat_image_source_cache (
    room_id TEXT PRIMARY KEY,
    source_url TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS message_cache_state (
    room_id TEXT PRIMARY KEY,
    next_from TEXT,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS message_cache (
    room_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    event_id TEXT,
    in_reply_to_event_id TEXT,
    sender TEXT NOT NULL,
    timestamp INTEGER,
    body TEXT NOT NULL,
    formatted_body TEXT,
    message_type TEXT,
    image_url TEXT,
    thumbnail_url TEXT,
    reactions TEXT,
    custom_emojis TEXT,
    encrypted INTEGER NOT NULL,
    decryption_status TEXT NOT NULL,
    verification_status TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (room_id, sequence)
);
