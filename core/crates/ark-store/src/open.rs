use rusqlite::Connection;

pub fn open_db(path: &str) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

pub fn check_integrity(connection: &Connection) -> Result<(), String> {
    let result: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("integrity_check query failed: {error}"))?;
    if result != "ok" {
        return Err(format!("ARK DB integrity check failed: {result}"));
    }
    Ok(())
}

/// Initializes the shared database schema and Store-owned prerequisites needed
/// before the canonical migration plan can read the database.
pub fn init_schema_prerequisites(connection: &Connection) -> Result<(), String> {
    check_integrity(connection)?;
    connection
        .pragma_update(None, "auto_vacuum", "FULL")
        .map_err(|error| error.to_string())?;
    connection
        .execute_batch(crate::schema::CREATE_TABLES)
        .map_err(|error| error.to_string())?;
    crate::schema::migrations::ensure_database_compatibility(connection)?;
    connection
        .execute_batch("SAVEPOINT ark_phase2_init")
        .map_err(|error| error.to_string())?;
    let result = (|| {
        crate::type_registry::migrate_schema(connection)?;
        crate::data_platform::ensure_schema(connection)?;
        if crate::objects::ensure_object_search_fts(connection).is_ok() {
            crate::objects::rebuild_object_search_fts(connection)?;
        }
        Ok::<(), String>(())
    })();
    match result {
        Ok(()) => connection
            .execute_batch("RELEASE ark_phase2_init")
            .map_err(|error| error.to_string()),
        Err(error) => {
            let _ = connection.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_phase2_init; RELEASE SAVEPOINT ark_phase2_init",
            );
            Err(error)
        }
    }
}

/// Initialize the complete ARK schema before the database actor becomes ready.
pub fn init_schema(connection: &Connection) -> Result<(), String> {
    init_schema_prerequisites(connection)?;
    crate::canonical_migrations::objects::ensure_object_state_schema(connection).map_err(
        |error| format!("canonical runtime schema during init_schema failed: {error:?}"),
    )?;
    let migration = crate::canonical_migrations::migration::migrate_phase3(connection)
        .map_err(|error| format!("phase3 migration during init_schema failed: {error:?}"))?;
    crate::canonical_migrations::registry::prepare_registry_for_objects(connection).map_err(
        |error| format!("canonical version install during init_schema failed: {error:?}"),
    )?;
    if migration.status == "completed" {
        crate::canonical_migrations::migration::retire_legacy_planning_tables(connection)
            .map(|_| ())
            .map_err(|error| {
                format!("legacy planning retirement during init_schema failed: {error:?}")
            })?;
    }
    Ok(())
}

pub fn backup_to_file_chunked_unpublished(
    source_path: &str,
    destination_path: &str,
    pages_per_step: i32,
    pause: std::time::Duration,
) -> Result<(), String> {
    let source = open_db(source_path)?;
    let mut destination = Connection::open(destination_path)
        .map_err(|error| format!("backup_to_file_chunked open dest failed: {error}"))?;
    let backup = rusqlite::backup::Backup::new(&source, &mut destination)
        .map_err(|error| format!("backup_to_file_chunked init failed: {error}"))?;
    backup
        .run_to_completion(
            pages_per_step,
            pause,
            None::<fn(rusqlite::backup::Progress)>,
        )
        .map_err(|error| format!("backup_to_file_chunked run failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{check_integrity, init_schema_prerequisites, open_db};
    use rusqlite::Connection;

    #[test]
    fn reopened_database_keeps_durable_foreign_key_settings() {
        let directory = tempfile::tempdir().expect("temporary directory opens");
        let path = directory.path().join("ark.db");
        let path = path.to_str().expect("temporary path is UTF-8");

        for _ in 0..2 {
            let connection = open_db(path).expect("database opens");
            let journal: String = connection
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .expect("journal mode reads");
            let synchronous: i64 = connection
                .query_row("PRAGMA synchronous", [], |row| row.get(0))
                .expect("synchronous mode reads");
            let foreign_keys: i64 = connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
                .expect("foreign-key mode reads");
            let busy_timeout_ms: i64 = connection
                .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
                .expect("busy timeout reads");

            assert_eq!(journal, "wal");
            assert_eq!(synchronous, 2);
            assert_eq!(foreign_keys, 1);
            assert_eq!(busy_timeout_ms, 5000);
        }
    }

    #[test]
    fn integrity_check_accepts_a_valid_database() {
        let connection = Connection::open_in_memory().expect("database opens");
        check_integrity(&connection).expect("fresh database passes integrity check");
    }

    #[test]
    fn phase_two_schema_bootstrap_is_idempotent() {
        let connection = Connection::open_in_memory().expect("database opens");

        init_schema_prerequisites(&connection).expect("initial schema bootstraps");
        init_schema_prerequisites(&connection).expect("schema reopens idempotently");

        for table in [
            "object_types",
            "object_type_versions",
            "sync_profiles",
            "accepted_peer_sync_policies",
        ] {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    [table],
                    |row| row.get(0),
                )
                .expect("schema table lookup succeeds");
            assert!(exists, "Store bootstrap creates {table}");
        }
    }
}
