//! Schema versioning for the encrypted application database.
//!
//! The version lives in SQLite's `user_version` header field. Every migration
//! runs inside one transaction that also writes the new version, so an
//! interrupted upgrade leaves the previous version and re-runs on next launch
//! rather than a half-migrated schema.

use rusqlite::{Connection, OptionalExtension};

/// The version this build expects. Bump it and append a migration to
/// [`MIGRATIONS`] whenever the schema changes.
pub const SCHEMA_VERSION: i64 = 1;

/// Applied in ascending order. Each entry is `(version, sql)`: a database at
/// version *n* runs every entry whose version is greater than *n*.
const MIGRATIONS: &[(i64, &str)] = &[(1, include_str!("migrations/001_initial.sql"))];

/// Brings `connection` up to [`SCHEMA_VERSION`].
///
/// A database that reports a version this build does not know about is
/// recreated empty rather than guessed at: every table here is a cache or a
/// re-derivable session, so discarding one costs a re-fetch, whereas
/// interpreting an unknown layout can corrupt it. A database with no
/// `user_version` at all is a pre-versioning file and takes the same path.
pub fn migrate(connection: &Connection) -> Result<(), String> {
    let current = read_version(connection)?;

    if current > SCHEMA_VERSION {
        log::warn!(
            "App database schema version {current} is newer than this build's {SCHEMA_VERSION}; recreating it"
        );
        recreate(connection)?;
        return Ok(());
    }

    if current == SCHEMA_VERSION {
        return Ok(());
    }

    let pending: Vec<(i64, &str)> = MIGRATIONS
        .iter()
        .filter(|(version, _)| *version > current)
        .map(|(version, sql)| (*version, *sql))
        .collect();

    apply(connection, &pending)
}

/// Runs `chain` in order, each entry in its own transaction that also records
/// the version, so a failure leaves the schema and the version consistent.
fn apply(connection: &Connection, chain: &[(i64, &str)]) -> Result<(), String> {
    for (version, sql) in chain {
        log::info!("Migrating app database schema to version {version}");

        let transaction = connection
            .unchecked_transaction()
            .map_err(|error| format!("Failed to begin schema migration: {error}"))?;

        transaction
            .execute_batch(sql)
            .map_err(|error| format!("Failed to apply schema migration {version}: {error}"))?;

        // user_version is a header field, so it participates in the
        // transaction: a failure here rolls the schema change back with it.
        transaction
            .pragma_update(None, "user_version", *version)
            .map_err(|error| {
                format!("Failed to record app database schema version {version}: {error}")
            })?;

        transaction
            .commit()
            .map_err(|error| format!("Failed to commit schema migration {version}: {error}"))?;
    }

    Ok(())
}

fn read_version(connection: &Connection) -> Result<i64, String> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .optional()
        .map_err(|error| format!("Failed to read app database schema version: {error}"))?
        .ok_or_else(|| String::from("App database did not report a schema version"))
}

/// Drops every table and index in the file, then re-runs the full chain.
///
/// The file's own `sqlite_master` is the authority here: a hand-kept list of
/// owned tables would go stale the moment a migration adds one, and a table
/// left behind by a layout this build cannot read is worse than no table.
fn recreate(connection: &Connection) -> Result<(), String> {
    let targets: Vec<String> = {
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE type IN ('table', 'index') AND name NOT LIKE 'sqlite_%'")
            .map_err(|error| format!("Failed to read app database objects: {error}"))?;

        let mut rows = statement
            .query([])
            .map_err(|error| format!("Failed to query app database objects: {error}"))?;

        let mut names = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|error| format!("Failed to read app database object name: {error}"))?
        {
            names.push(
                row.get::<_, String>(0).map_err(|error| {
                    format!("Failed to decode app database object name: {error}")
                })?,
            );
        }
        names
    };

    for target in targets {
        connection
            .execute(&format!("DROP TABLE IF EXISTS \"{target}\""), [])
            .map_err(|error| format!("Failed to drop app database object {target}: {error}"))?;
    }

    connection
        .pragma_update(None, "user_version", 0)
        .map_err(|error| format!("Failed to reset app database schema version: {error}"))?;

    migrate(connection)
}

#[cfg(test)]
mod tests {
    use super::{migrate, SCHEMA_VERSION};
    use rusqlite::Connection;

    fn version(connection: &Connection) -> i64 {
        connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("user_version")
    }

    fn table_exists(connection: &Connection, name: &str) -> bool {
        connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [name],
                |row| row.get::<_, i64>(0),
            )
            .expect("sqlite_master")
            > 0
    }

    #[test]
    fn a_fresh_database_lands_on_the_current_version() {
        let connection = Connection::open_in_memory().expect("open");

        migrate(&connection).expect("migrate");

        assert_eq!(version(&connection), SCHEMA_VERSION);
        assert!(table_exists(&connection, "message_cache"));
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let connection = Connection::open_in_memory().expect("open");

        migrate(&connection).expect("first migrate");
        migrate(&connection).expect("second migrate");

        assert_eq!(version(&connection), SCHEMA_VERSION);
    }

    #[test]
    fn a_database_from_the_future_is_recreated() {
        let connection = Connection::open_in_memory().expect("open");
        migrate(&connection).expect("migrate");

        connection
            .execute_batch(
                "
                PRAGMA user_version = 9999;
                CREATE TABLE from_the_future (id INTEGER PRIMARY KEY);
                CREATE INDEX from_the_future_index ON from_the_future (id);
                ",
            )
            .expect("seed future schema");

        migrate(&connection).expect("migrate");

        assert_eq!(version(&connection), SCHEMA_VERSION);
        assert!(!table_exists(&connection, "from_the_future"));
        assert!(table_exists(&connection, "chats_cache"));
    }

    #[test]
    fn a_recreate_drops_objects_the_current_build_never_heard_of() {
        let connection = Connection::open_in_memory().expect("open");
        migrate(&connection).expect("migrate");

        connection
            .execute_batch(
                "
                PRAGMA user_version = 9999;
                CREATE TABLE undocumented (payload TEXT);
                CREATE INDEX undocumented_payload ON undocumented (payload);
                ",
            )
            .expect("seed future schema");

        migrate(&connection).expect("migrate");

        let leftovers: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'undocumented_payload'",
                [],
                |row| row.get(0),
            )
            .expect("sqlite_master");

        assert_eq!(leftovers, 0);
    }

    #[test]
    fn a_pre_versioning_database_is_recreated() {
        let connection = Connection::open_in_memory().expect("open");

        // A file from before user_version was tracked: tables exist, no version.
        connection
            .execute_batch(
                "
                CREATE TABLE chats_cache (room_id TEXT PRIMARY KEY, display_name TEXT NOT NULL);
                CREATE TABLE message_cache (room_id TEXT NOT NULL);
                ",
            )
            .expect("seed legacy schema");

        migrate(&connection).expect("migrate");

        assert_eq!(version(&connection), SCHEMA_VERSION);
        assert!(table_exists(&connection, "chats_cache"));
    }

    #[test]
    fn a_versioned_upgrade_applies_every_pending_migration() {
        let connection = Connection::open_in_memory().expect("open");
        // A database pinned below the current version, with nothing in it.
        connection
            .pragma_update(None, "user_version", 0i64)
            .expect("pin version");

        migrate(&connection).expect("migrate");

        assert_eq!(version(&connection), SCHEMA_VERSION);
        assert!(table_exists(&connection, "message_cache_state"));
    }

    #[test]
    fn a_failing_migration_leaves_the_previous_version() {
        use super::apply;

        let connection = Connection::open_in_memory().expect("open");
        connection
            .pragma_update(None, "user_version", 0i64)
            .expect("pin");

        let error = apply(
            &connection,
            &[(1, "CREATE TABLE a (id INTEGER);"), (2, "not valid sql;")],
        )
        .expect_err("migration should fail");

        assert!(
            error.contains("Failed to apply schema migration 2"),
            "{error}"
        );
        // Version 1 committed, so a retry resumes from 2 rather than redoing 1.
        assert_eq!(version(&connection), 1);
        assert!(table_exists(&connection, "a"));
    }

    #[test]
    fn a_failing_first_migration_changes_nothing() {
        use super::apply;

        let connection = Connection::open_in_memory().expect("open");
        connection
            .pragma_update(None, "user_version", 0i64)
            .expect("pin");

        let error = apply(&connection, &[(1, "not valid sql;")]).expect_err("should fail");

        assert!(
            error.contains("Failed to apply schema migration 1"),
            "{error}"
        );
        assert_eq!(version(&connection), 0);
        assert!(!table_exists(&connection, "message_cache"));
    }
}
