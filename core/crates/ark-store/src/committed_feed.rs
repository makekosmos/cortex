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

pub fn bootstrap_committed_feed_heads(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT ark_feed_heads_bootstrap")
        .map_err(|error| error.to_string())?;
    let result = (|| -> Result<(), String> {
        let bootstrapped = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM ark_committed_feed_meta
                 WHERE meta_key = 'heads-v1')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?
            != 0;
        if !bootstrapped {
            conn.execute(
                "INSERT INTO ark_committed_feed_heads(entity_type, entity_id, position)
                 SELECT entity_type, entity_id, MAX(position)
                 FROM ark_committed_feed WHERE 1 GROUP BY entity_type, entity_id
                 ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                     position = MAX(ark_committed_feed_heads.position, excluded.position)",
                [],
            )
            .map_err(|error| error.to_string())?;
            conn.execute(
                "INSERT INTO ark_committed_feed_meta(meta_key, meta_value)
                 VALUES ('heads-v1', 'complete')",
                [],
            )
            .map_err(|error| error.to_string())?;
        }
        let watermark = watermark(conn)?;
        conn.execute(
            "INSERT INTO ark_committed_feed_meta(meta_key, meta_value)
             VALUES ('watermark-v1', ?1)
             ON CONFLICT(meta_key) DO UPDATE SET
                 meta_value = MAX(CAST(ark_committed_feed_meta.meta_value AS INTEGER),
                                  CAST(excluded.meta_value AS INTEGER))",
            [watermark.to_string()],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE ark_feed_heads_bootstrap")
            .map_err(|error| error.to_string()),
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO ark_feed_heads_bootstrap; RELEASE ark_feed_heads_bootstrap",
            );
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bootstrap_committed_feed_heads, watermark};
    use rusqlite::Connection;

    #[test]
    fn feed_heads_bootstrap_legacy_rows_once_and_is_idempotent() {
        let conn = Connection::open_in_memory().expect("database opens");
        conn.execute_batch(crate::schema::CREATE_TABLES)
            .expect("store schema initializes");
        conn.execute(
            "INSERT INTO ark_committed_feed(
                position, entity_type, entity_id, revision, deleted
             ) VALUES (1, 'object', 'legacy', 'r1', 0),
                      (2, 'object', 'legacy', 'r2', 0),
                      (3, 'todo', 'other', 'r3', 0)",
            [],
        )
        .expect("feed fixture inserts");

        bootstrap_committed_feed_heads(&conn).expect("legacy feed heads bootstrap");
        let first: Vec<(String, String, i64)> = conn
            .prepare(
                "SELECT entity_type, entity_id, position FROM ark_committed_feed_heads
                 ORDER BY entity_type, entity_id",
            )
            .expect("feed heads load")
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("feed head query executes")
            .collect::<Result<_, _>>()
            .expect("feed head rows load");
        assert_eq!(
            first,
            vec![
                ("object".into(), "legacy".into(), 2),
                ("todo".into(), "other".into(), 3)
            ]
        );

        conn.execute(
            "UPDATE ark_committed_feed_heads SET position = 1
             WHERE entity_type = 'object' AND entity_id = 'legacy'",
            [],
        )
        .expect("feed fixture update");
        bootstrap_committed_feed_heads(&conn).expect("idempotent feed heads bootstrap");
        let position: i64 = conn
            .query_row(
                "SELECT position FROM ark_committed_feed_heads
                 WHERE entity_type = 'object' AND entity_id = 'legacy'",
                [],
                |row| row.get(0),
            )
            .expect("bootstrapped feed head loads");
        assert_eq!(
            position, 1,
            "the completion marker prevents a second rebuild"
        );
    }

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
