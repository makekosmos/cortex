use rusqlite::{params, Connection, OptionalExtension};

pub fn get_sync_kv(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM sync_kv WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn set_sync_kv(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO sync_kv (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{get_sync_kv, set_sync_kv};
    use rusqlite::Connection;

    #[test]
    fn reads_and_replaces_persisted_values() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch("CREATE TABLE sync_kv (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            .expect("key-value schema initializes");

        assert_eq!(
            get_sync_kv(&connection, "peer").expect("missing key reads as absent"),
            None
        );
        set_sync_kv(&connection, "peer", "first").expect("first value writes");
        set_sync_kv(&connection, "peer", "second").expect("second value replaces first");

        assert_eq!(
            get_sync_kv(&connection, "peer")
                .expect("persisted value reads")
                .as_deref(),
            Some("second")
        );
    }
}
