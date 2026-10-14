-- Schema version 2: avatar change propagation.
--
-- The latest avatar mxc observed per user, from m.room.member events. The
-- SDK's member profile can lag a sync behind, so get_user_avatar prefers this
-- fresher source over the stored member profile.

CREATE TABLE IF NOT EXISTS user_avatar_source_cache (
    user_id TEXT PRIMARY KEY,
    source_url TEXT,
    updated_at INTEGER NOT NULL
);
