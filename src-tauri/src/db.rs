use std::fs;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use matrix_sdk::authentication::matrix::MatrixSession as SdkMatrixSession;
use rusqlite::{params, Connection, OptionalExtension};
use tauri::AppHandle;

use crate::messages::{
    MatrixChatMessage, MatrixGetChatMessagesResponse, MatrixMessageDecryptionStatus,
    MatrixMessageVerificationStatus,
};
use crate::protocol::storage_keys;
use crate::rooms::MatrixChatSummary;
use crate::storage;

const SINGLETON_ROW_ID: i64 = 1;

pub struct AppDb {
    connection: Mutex<Connection>,
}

impl AppDb {
    pub(crate) fn initialize(app: &AppHandle) -> Result<Self, String> {
        let path = storage::app_data_file(app, storage_keys::APP_DB_FILE)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create app data directory: {error}"))?;
        }

        let secret = storage::get_or_create_keyring_secret(
            app,
            storage_keys::KEYCHAIN_SERVICE,
            storage_keys::KEYCHAIN_APP_DB_KEY,
            32,
        )?;

        let connection = match Self::open_encrypted_connection(&path, &secret) {
            Ok(connection) => connection,
            Err(error) => {
                if path.exists() && is_database_key_mismatch(&error) {
                    log::warn!(
                        "Encrypted app cache could not be opened with current key; recreating app cache database"
                    );
                    fs::remove_file(&path).map_err(|remove_error| {
                        format!("Failed to reset app database file: {remove_error}")
                    })?;
                    Self::open_encrypted_connection(&path, &secret)?
                } else {
                    return Err(error);
                }
            }
        };

        connection
            .execute_batch(
                "
                DROP TABLE IF EXISTS app_cache;

                CREATE TABLE IF NOT EXISTS session_cache (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    homeserver_url TEXT NOT NULL,
                    matrix_session BLOB NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS chats_cache (
                    room_id TEXT PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    encrypted INTEGER NOT NULL,
                    joined_members INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                DROP TABLE IF EXISTS message_cache_state;
                DROP TABLE IF EXISTS message_cache;

                CREATE TABLE IF NOT EXISTS message_cache_state (
                    room_id TEXT PRIMARY KEY,
                    next_from TEXT,
                    updated_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS message_cache (
                    room_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL,
                    event_id TEXT,
                    sender TEXT NOT NULL,
                    timestamp INTEGER,
                    body TEXT NOT NULL,
                    message_type TEXT,
                    image_url TEXT,
                    video_thumbnail_url TEXT,
                    video_mime_type TEXT,
                    video_size_bytes INTEGER,
                    video_duration_ms INTEGER,
                    encrypted INTEGER NOT NULL,
                    decryption_status TEXT NOT NULL,
                    verification_status TEXT NOT NULL,
                    updated_at INTEGER NOT NULL,
                    PRIMARY KEY (room_id, sequence)
                );
                ",
            )
            .map_err(|error| format!("Failed to initialize app database schema: {error}"))?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn open_encrypted_connection(
        path: &std::path::Path,
        secret: &str,
    ) -> Result<Connection, String> {
        let connection = Connection::open(path)
            .map_err(|error| format!("Failed to open app database: {error}"))?;

        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| format!("Failed to configure app database busy timeout: {error}"))?;

        connection
            .pragma_update(None, "key", secret)
            .map_err(|error| format!("Failed to unlock encrypted app database: {error}"))?;

        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|error| format!("Failed to enable app database foreign keys: {error}"))?;

        connection
            .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|error| format!("Failed to verify encrypted app database access: {error}"))?;

        Ok(connection)
    }

    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.connection
            .lock()
            .map_err(|_| String::from("Failed to lock app database connection"))
    }

    pub(crate) fn persist_session(
        &self,
        homeserver_url: &str,
        matrix_session: &SdkMatrixSession,
    ) -> Result<(), String> {
        let serialized_session = rmp_serde::to_vec(matrix_session)
            .map_err(|error| format!("Failed to encode Matrix session: {error}"))?;

        let connection = self.lock()?;
        connection
            .execute(
                "
                INSERT INTO session_cache (id, homeserver_url, matrix_session, updated_at)
                VALUES (?1, ?2, ?3, unixepoch())
                ON CONFLICT(id) DO UPDATE SET
                    homeserver_url = excluded.homeserver_url,
                    matrix_session = excluded.matrix_session,
                    updated_at = unixepoch()
                ",
                params![SINGLETON_ROW_ID, homeserver_url, serialized_session],
            )
            .map_err(|error| format!("Failed to persist session cache entry: {error}"))?;

        Ok(())
    }

    pub(crate) fn load_persisted_session(
        &self,
    ) -> Result<Option<(String, SdkMatrixSession)>, String> {
        let connection = self.lock()?;
        let row = connection
            .query_row(
                "SELECT homeserver_url, matrix_session FROM session_cache WHERE id = ?1",
                [SINGLETON_ROW_ID],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()
            .map_err(|error| format!("Failed to read session cache entry: {error}"))?;

        let Some((homeserver_url, matrix_session_encoded)) = row else {
            return Ok(None);
        };

        let matrix_session: SdkMatrixSession = rmp_serde::from_slice(&matrix_session_encoded)
            .map_err(|error| format!("Failed to decode Matrix session: {error}"))?;

        Ok(Some((homeserver_url, matrix_session)))
    }

    pub(crate) fn clear_session(&self) -> Result<(), String> {
        let connection = self.lock()?;
        connection
            .execute(
                "DELETE FROM session_cache WHERE id = ?1",
                [SINGLETON_ROW_ID],
            )
            .map_err(|error| format!("Failed to delete session cache entry: {error}"))?;
        Ok(())
    }

    pub(crate) fn store_chats(&self, chats: &[MatrixChatSummary]) -> Result<(), String> {
        let mut connection = self.lock()?;
        let tx = connection
            .transaction()
            .map_err(|error| format!("Failed to start chats cache transaction: {error}"))?;

        tx.execute("DELETE FROM chats_cache", [])
            .map_err(|error| format!("Failed to clear chats cache: {error}"))?;

        {
            let mut statement = tx
                .prepare(
                    "
                    INSERT INTO chats_cache (room_id, display_name, encrypted, joined_members, updated_at)
                    VALUES (?1, ?2, ?3, ?4, unixepoch())
                    ",
                )
                .map_err(|error| format!("Failed to prepare chats cache insert: {error}"))?;

            for chat in chats {
                statement
                    .execute(params![
                        chat.room_id,
                        chat.display_name,
                        if chat.encrypted { 1_i64 } else { 0_i64 },
                        chat.joined_members as i64,
                    ])
                    .map_err(|error| format!("Failed to insert chats cache row: {error}"))?;
            }
        }

        tx.commit()
            .map_err(|error| format!("Failed to commit chats cache transaction: {error}"))?;

        Ok(())
    }

    pub(crate) fn load_cached_chats(&self) -> Result<Option<Vec<MatrixChatSummary>>, String> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "
                SELECT room_id, display_name, encrypted, joined_members
                FROM chats_cache
                ORDER BY updated_at DESC, room_id ASC
                ",
            )
            .map_err(|error| format!("Failed to prepare chats cache query: {error}"))?;

        let mut rows = statement
            .query([])
            .map_err(|error| format!("Failed to query chats cache: {error}"))?;

        let mut chats = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|error| format!("Failed to read chats cache row: {error}"))?
        {
            let encrypted_flag: i64 = row
                .get(2)
                .map_err(|error| format!("Failed to decode chats cache encrypted flag: {error}"))?;
            let joined_members_raw: i64 = row
                .get(3)
                .map_err(|error| format!("Failed to decode chats cache joined members: {error}"))?;

            chats.push(MatrixChatSummary {
                room_id: row
                    .get::<_, String>(0)
                    .map_err(|error| format!("Failed to decode chats cache room id: {error}"))?,
                display_name: row.get::<_, String>(1).map_err(|error| {
                    format!("Failed to decode chats cache display name: {error}")
                })?,
                encrypted: encrypted_flag != 0,
                joined_members: joined_members_raw.max(0) as u64,
            });
        }

        if chats.is_empty() {
            return Ok(None);
        }

        Ok(Some(chats))
    }

    pub(crate) fn store_initial_room_messages(
        &self,
        response: &MatrixGetChatMessagesResponse,
    ) -> Result<(), String> {
        let room_id = response.room_id.as_str();

        let mut connection = self.lock()?;
        let tx = connection
            .transaction()
            .map_err(|error| format!("Failed to start message cache transaction: {error}"))?;

        tx.execute(
            "
            INSERT INTO message_cache_state (room_id, next_from, updated_at)
            VALUES (?1, ?2, unixepoch())
            ON CONFLICT(room_id) DO UPDATE SET
                next_from = excluded.next_from,
                updated_at = unixepoch()
            ",
            params![room_id, response.next_from.as_deref()],
        )
        .map_err(|error| format!("Failed to upsert message cache state: {error}"))?;

        tx.execute("DELETE FROM message_cache WHERE room_id = ?1", [room_id])
            .map_err(|error| format!("Failed to clear message cache rows: {error}"))?;

        {
            let mut statement = tx
                .prepare(
                    "
                    INSERT INTO message_cache (
                        room_id,
                        sequence,
                        event_id,
                        sender,
                        timestamp,
                        body,
                        message_type,
                        image_url,
                        video_thumbnail_url,
                        video_mime_type,
                        video_size_bytes,
                        video_duration_ms,
                        encrypted,
                        decryption_status,
                        verification_status,
                        updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, unixepoch())
                    ",
                )
                .map_err(|error| format!("Failed to prepare message cache insert: {error}"))?;

            for (index, message) in response.messages.iter().enumerate() {
                statement
                    .execute(params![
                        room_id,
                        index as i64,
                        message.event_id.as_deref(),
                        message.sender,
                        message.timestamp.map(|value| value as i64),
                        message.body,
                        message.message_type.as_deref(),
                        message.image_url.as_deref(),
                        message.video_thumbnail_url.as_deref(),
                        message.video_mime_type.as_deref(),
                        message.video_size_bytes.map(|value| value as i64),
                        message.video_duration_ms.map(|value| value as i64),
                        if message.encrypted { 1_i64 } else { 0_i64 },
                        decryption_status_to_db(message.decryption_status),
                        verification_status_to_db(message.verification_status),
                    ])
                    .map_err(|error| format!("Failed to insert message cache row: {error}"))?;
            }
        }

        tx.commit()
            .map_err(|error| format!("Failed to commit message cache transaction: {error}"))?;

        Ok(())
    }

    pub(crate) fn load_initial_room_messages(
        &self,
        room_id: &str,
    ) -> Result<Option<MatrixGetChatMessagesResponse>, String> {
        let connection = self.lock()?;
        let next_from = connection
            .query_row(
                "SELECT next_from FROM message_cache_state WHERE room_id = ?1",
                [room_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|error| format!("Failed to read message cache state: {error}"))?;

        let Some(next_from) = next_from else {
            return Ok(None);
        };

        let mut statement = connection
            .prepare(
                "
                SELECT
                    event_id,
                    sender,
                    timestamp,
                    body,
                    message_type,
                    image_url,
                    video_thumbnail_url,
                    video_mime_type,
                    video_size_bytes,
                    video_duration_ms,
                    encrypted,
                    decryption_status,
                    verification_status
                FROM message_cache
                WHERE room_id = ?1
                ORDER BY sequence ASC
                ",
            )
            .map_err(|error| format!("Failed to prepare message cache query: {error}"))?;

        let mut rows = statement
            .query([room_id])
            .map_err(|error| format!("Failed to query message cache rows: {error}"))?;

        let mut messages = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|error| format!("Failed to read message cache row: {error}"))?
        {
            let timestamp_raw: Option<i64> = row
                .get(2)
                .map_err(|error| format!("Failed to decode cached timestamp: {error}"))?;
            let video_size_bytes_raw: Option<i64> = row
                .get(8)
                .map_err(|error| format!("Failed to decode cached video size bytes: {error}"))?;
            let video_duration_ms_raw: Option<i64> = row
                .get(9)
                .map_err(|error| format!("Failed to decode cached video duration ms: {error}"))?;
            let encrypted_flag: i64 = row
                .get(10)
                .map_err(|error| format!("Failed to decode cached encrypted flag: {error}"))?;
            let decryption_status_raw: String = row
                .get(11)
                .map_err(|error| format!("Failed to decode cached decryption status: {error}"))?;
            let verification_status_raw: String = row
                .get(12)
                .map_err(|error| format!("Failed to decode cached verification status: {error}"))?;

            messages.push(MatrixChatMessage {
                event_id: row
                    .get::<_, Option<String>>(0)
                    .map_err(|error| format!("Failed to decode cached event id: {error}"))?,
                sender: row
                    .get::<_, String>(1)
                    .map_err(|error| format!("Failed to decode cached sender: {error}"))?,
                timestamp: timestamp_raw.map(|value| value.max(0) as u64),
                body: row
                    .get::<_, String>(3)
                    .map_err(|error| format!("Failed to decode cached body: {error}"))?,
                message_type: row
                    .get::<_, Option<String>>(4)
                    .map_err(|error| format!("Failed to decode cached message type: {error}"))?,
                image_url: row
                    .get::<_, Option<String>>(5)
                    .map_err(|error| format!("Failed to decode cached image url: {error}"))?,
                video_thumbnail_url: row
                    .get::<_, Option<String>>(6)
                    .map_err(|error| {
                        format!("Failed to decode cached video thumbnail url: {error}")
                    })?,
                video_mime_type: row
                    .get::<_, Option<String>>(7)
                    .map_err(|error| format!("Failed to decode cached video mime type: {error}"))?,
                video_size_bytes: video_size_bytes_raw.map(|value| value.max(0) as u64),
                video_duration_ms: video_duration_ms_raw.map(|value| value.max(0) as u64),
                encrypted: encrypted_flag != 0,
                decryption_status: decryption_status_from_db(&decryption_status_raw)?,
                verification_status: verification_status_from_db(&verification_status_raw)?,
            });
        }

        Ok(Some(MatrixGetChatMessagesResponse {
            room_id: room_id.to_owned(),
            next_from,
            messages,
        }))
    }

    pub(crate) fn clear_app_cache(&self) -> Result<(), String> {
        let connection = self.lock()?;
        connection
            .execute("DELETE FROM session_cache", [])
            .map_err(|error| format!("Failed to clear session cache: {error}"))?;
        connection
            .execute("DELETE FROM chats_cache", [])
            .map_err(|error| format!("Failed to clear chats cache: {error}"))?;
        connection
            .execute("DELETE FROM message_cache_state", [])
            .map_err(|error| format!("Failed to clear message cache state: {error}"))?;
        connection
            .execute("DELETE FROM message_cache", [])
            .map_err(|error| format!("Failed to clear message cache: {error}"))?;
        Ok(())
    }

    pub(crate) fn clear_non_auth_cache(&self) -> Result<(), String> {
        let connection = self.lock()?;
        connection
            .execute("DELETE FROM chats_cache", [])
            .map_err(|error| format!("Failed to clear chats cache: {error}"))?;
        connection
            .execute("DELETE FROM message_cache_state", [])
            .map_err(|error| format!("Failed to clear message cache state: {error}"))?;
        connection
            .execute("DELETE FROM message_cache", [])
            .map_err(|error| format!("Failed to clear message cache: {error}"))?;
        Ok(())
    }
}

fn is_database_key_mismatch(error: &str) -> bool {
    error.contains("file is not a database")
        || error.contains("database disk image is malformed")
        || error.contains("not an error")
}

fn decryption_status_to_db(status: MatrixMessageDecryptionStatus) -> &'static str {
    match status {
        MatrixMessageDecryptionStatus::Plaintext => "plaintext",
        MatrixMessageDecryptionStatus::Decrypted => "decrypted",
        MatrixMessageDecryptionStatus::UnableToDecrypt => "unable_to_decrypt",
    }
}

fn decryption_status_from_db(status: &str) -> Result<MatrixMessageDecryptionStatus, String> {
    match status {
        "plaintext" => Ok(MatrixMessageDecryptionStatus::Plaintext),
        "decrypted" => Ok(MatrixMessageDecryptionStatus::Decrypted),
        "unable_to_decrypt" => Ok(MatrixMessageDecryptionStatus::UnableToDecrypt),
        _ => Err(format!("Invalid decryption status in cache: {status}")),
    }
}

fn verification_status_to_db(status: MatrixMessageVerificationStatus) -> &'static str {
    match status {
        MatrixMessageVerificationStatus::Unknown => "unknown",
        MatrixMessageVerificationStatus::Verified => "verified",
        MatrixMessageVerificationStatus::Unverified => "unverified",
    }
}

fn verification_status_from_db(status: &str) -> Result<MatrixMessageVerificationStatus, String> {
    match status {
        "unknown" => Ok(MatrixMessageVerificationStatus::Unknown),
        "verified" => Ok(MatrixMessageVerificationStatus::Verified),
        "unverified" => Ok(MatrixMessageVerificationStatus::Unverified),
        _ => Err(format!("Invalid verification status in cache: {status}")),
    }
}
