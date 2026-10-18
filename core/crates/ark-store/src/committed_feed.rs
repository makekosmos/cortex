use rusqlite::{Connection, OptionalExtension};

pub fn watermark(connection: &Connection) -> Result<i64, String> {
    let persisted: Option<String> = connection
        .query_row(
            "SELECT meta_value FROM ark_committed_feed_meta WHERE meta_key = 'watermark-v1'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let persisted = persisted
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| "invalid_committed_feed_watermark".to_string())
        })
        .transpose()?
        .unwrap_or_default();
    if persisted < 0 {
        return Err("invalid_committed_feed_watermark".into());
    }

    // AUTOINCREMENT preserves a lower bound after feed pruning. Retained rows
    // cover databases created before the sequence and watermark were stored.
    let retained: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(position), 0) FROM ark_committed_feed",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let sequence: Option<i64> = connection
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name = 'ark_committed_feed'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(persisted.max(retained).max(sequence.unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::watermark;
    use rusqlite::Connection;

    #[test]
    fn watermark_survives_feed_pruning_and_reopen() {
        let directory = tempfile::tempdir().expect("temporary directory opens");
        let path = directory.path().join("ark.db");
        let connection = Connection::open(&path).expect("database opens");
        connection
            .execute_batch(
                "CREATE TABLE ark_committed_feed (
                    position INTEGER PRIMARY KEY AUTOINCREMENT,
                    entity_id TEXT NOT NULL
                 );
                 CREATE TABLE ark_committed_feed_meta (
                    meta_key TEXT PRIMARY KEY,
                    meta_value TEXT NOT NULL
                 );
                 INSERT INTO ark_committed_feed(entity_id) VALUES ('a'), ('b');
                 DELETE FROM ark_committed_feed;
                 INSERT INTO ark_committed_feed_meta VALUES ('watermark-v1', '2');",
            )
            .expect("feed schema and pruning initialize");
        drop(connection);

        let connection = Connection::open(&path).expect("database reopens");
        assert_eq!(watermark(&connection).expect("watermark loads"), 2);
    }
}
