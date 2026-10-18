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
    use super::{check_integrity, open_db};
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
}
