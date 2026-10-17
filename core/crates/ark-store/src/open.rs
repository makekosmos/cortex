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

#[cfg(test)]
mod tests {
    use super::open_db;

    #[test]
    fn reopened_database_keeps_durable_foreign_key_settings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ark.db");
        let path = path.to_str().unwrap();

        for _ in 0..2 {
            let connection = open_db(path).unwrap();
            let journal: String = connection
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .unwrap();
            let synchronous: i64 = connection
                .query_row("PRAGMA synchronous", [], |row| row.get(0))
                .unwrap();
            let foreign_keys: i64 = connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
                .unwrap();
            let busy_timeout_ms: i64 = connection
                .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
                .unwrap();

            assert_eq!(journal, "wal");
            assert_eq!(synchronous, 2);
            assert_eq!(foreign_keys, 1);
            assert_eq!(busy_timeout_ms, 5000);
        }
    }
}
