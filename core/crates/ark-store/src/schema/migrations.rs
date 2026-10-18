use rusqlite::Connection;

pub fn ensure_database_compatibility(conn: &Connection) -> Result<(), String> {
    ensure_task_merge_delivery_journal(conn)?;
    ensure_device_registry_position(conn)?;
    ensure_device_registry_updated_hlc(conn)?;
    crate::committed_feed::bootstrap_committed_feed_heads(conn)?;
    ensure_integration_credential_fence_column(conn)?;
    ensure_authorized_node_transport_key(conn)?;
    ensure_sync_tombstone_type_id(conn)?;
    ensure_usage_runtime_ms(conn)?;
    Ok(())
}

fn ensure_task_merge_delivery_journal(conn: &Connection) -> Result<(), String> {
    let columns = conn
        .prepare("PRAGMA table_info(task_merge_journal)")
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    if columns.iter().any(|column| column == "operation_id") {
        conn.execute_batch("SAVEPOINT ark_task_merge_delivery_journal")
            .map_err(|error| error.to_string())?;
        let migration = conn.execute_batch(
            "ALTER TABLE task_merge_journal RENAME TO task_merge_operation_journal_legacy;
             CREATE TABLE task_merge_journal (
                 position INTEGER PRIMARY KEY AUTOINCREMENT,
                 record_kind TEXT NOT NULL CHECK (record_kind IN ('bootstrap', 'operation')),
                 record_id TEXT NOT NULL,
                 UNIQUE(record_kind, record_id)
             );
             INSERT INTO task_merge_journal(position, record_kind, record_id)
             SELECT position, 'operation', operation_id
             FROM task_merge_operation_journal_legacy ORDER BY position;
             DROP TABLE task_merge_operation_journal_legacy;
             RELEASE SAVEPOINT ark_task_merge_delivery_journal;",
        );
        if let Err(error) = migration {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_task_merge_delivery_journal;
                 RELEASE SAVEPOINT ark_task_merge_delivery_journal;",
            );
            return Err(error.to_string());
        }
    }

    conn.execute_batch(
        "INSERT OR IGNORE INTO task_merge_journal(record_kind, record_id)
         SELECT 'operation', operation_id FROM task_merge_operations ORDER BY rowid;",
    )
    .map_err(|error| error.to_string())
}

fn ensure_device_registry_position(conn: &Connection) -> Result<(), String> {
    let has_position = conn
        .prepare("PRAGMA table_info(device_registry_events)")
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|name| name == "registry_position");
    if !has_position {
        conn.execute_batch(
            "ALTER TABLE device_registry_events
             ADD COLUMN registry_position INTEGER NOT NULL DEFAULT 0",
        )
        .map_err(|error| error.to_string())?;
        conn.execute_batch(
            "WITH ordered AS (
                 SELECT actor_device_id, actor_sequence,
                        row_number() OVER (ORDER BY actor_device_id, actor_sequence) AS position
                 FROM device_registry_events
             )
             UPDATE device_registry_events
             SET registry_position = (
                 SELECT position FROM ordered
                 WHERE ordered.actor_device_id = device_registry_events.actor_device_id
                   AND ordered.actor_sequence = device_registry_events.actor_sequence
             )",
        )
        .map_err(|error| error.to_string())?;
    }
    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_device_registry_events_position
         ON device_registry_events(registry_position)",
    )
    .map_err(|error| error.to_string())
}

fn ensure_device_registry_updated_hlc(conn: &Connection) -> Result<(), String> {
    let has_updated_hlc = conn
        .prepare("PRAGMA table_info(device_registry_projection)")
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|name| name == "updated_hlc");
    if !has_updated_hlc {
        conn.execute_batch("ALTER TABLE device_registry_projection ADD COLUMN updated_hlc TEXT")
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn ensure_authorized_node_transport_key(conn: &Connection) -> Result<(), String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('authorized_nodes')
             WHERE name = 'transport_public_key')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !exists {
        conn.execute(
            "ALTER TABLE authorized_nodes ADD COLUMN transport_public_key TEXT",
            [],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn ensure_integration_credential_fence_column(conn: &Connection) -> Result<(), String> {
    let has_column = conn
        .prepare("PRAGMA table_info(integration_credential_envelopes)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|name| name == "refresh_fencing_token");
    if !has_column {
        conn.execute_batch(
            "ALTER TABLE integration_credential_envelopes
             ADD COLUMN refresh_fencing_token INTEGER NOT NULL DEFAULT 0",
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn ensure_usage_runtime_ms(conn: &Connection) -> Result<(), String> {
    let has_runtime_ms = conn
        .prepare("PRAGMA table_info(usage_sessions)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|name| name == "runtime_ms");

    if !has_runtime_ms {
        conn.execute_batch(
            "ALTER TABLE usage_sessions ADD COLUMN runtime_ms INTEGER NOT NULL DEFAULT 0;",
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "UPDATE usage_sessions
         SET runtime_ms = COALESCE(foreground_ms, 0) + COALESCE(idle_ms, 0)
         WHERE runtime_ms = 0 AND (COALESCE(foreground_ms, 0) > 0 OR COALESCE(idle_ms, 0) > 0)",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_sync_tombstone_type_id(conn: &Connection) -> Result<(), String> {
    let has_type_id = conn
        .prepare("PRAGMA table_info(sync_tombstones)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .iter()
        .any(|name| name == "type_id");
    if !has_type_id {
        conn.execute_batch("ALTER TABLE sync_tombstones ADD COLUMN type_id TEXT;")
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_merge_journal_migration_preserves_positions_and_is_idempotent() {
        let conn = Connection::open_in_memory().expect("migration operation succeeds");
        conn.execute_batch(
            "CREATE TABLE task_merge_operations (
                 operation_id TEXT PRIMARY KEY,
                 object_id TEXT NOT NULL,
                 unit TEXT NOT NULL,
                 operation_json TEXT NOT NULL,
                 payload_sha256 TEXT NOT NULL
             );
             CREATE TABLE task_merge_journal (
                 position INTEGER PRIMARY KEY AUTOINCREMENT,
                 operation_id TEXT NOT NULL UNIQUE REFERENCES task_merge_operations(operation_id)
             );
             INSERT INTO task_merge_operations VALUES ('op-1', 'task-1', 'title', '{}', 'hash-1');
             INSERT INTO task_merge_operations VALUES ('op-2', 'task-1', 'title', '{}', 'hash-2');
             INSERT INTO task_merge_journal(position, operation_id) VALUES (4, 'op-1');
             INSERT INTO task_merge_journal(position, operation_id) VALUES (9, 'op-2');",
        )
        .expect("migration operation succeeds");

        ensure_task_merge_delivery_journal(&conn).expect("migration operation succeeds");
        ensure_task_merge_delivery_journal(&conn).expect("migration operation succeeds");

        let journal = conn
            .prepare(
                "SELECT position, record_kind, record_id FROM task_merge_journal
                 ORDER BY position",
            )
            .expect("migration operation succeeds")
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .expect("migration operation succeeds")
            .collect::<Result<Vec<_>, _>>()
            .expect("migration operation succeeds");
        assert_eq!(
            journal,
            vec![
                (4, "operation".into(), "op-1".into()),
                (9, "operation".into(), "op-2".into())
            ]
        );
    }
}
