// ---------------------------------------------------------------------------
// sync_kv
// ---------------------------------------------------------------------------

pub fn get_sync_kv(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM sync_kv WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn set_sync_kv(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO sync_kv (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn upsert_sync_tombstone(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
    let type_id = entity.data.get("typeId").and_then(|value| value.as_str());
    conn.execute(
        "INSERT INTO sync_tombstones (id, entity_type, hlc, deleted_at, type_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET
            entity_type = excluded.entity_type,
            hlc = excluded.hlc,
            deleted_at = excluded.deleted_at,
            type_id = COALESCE(excluded.type_id, sync_tombstones.type_id)
         WHERE excluded.hlc >= sync_tombstones.hlc",
        params![
            entity.id,
            entity.entity_type,
            entity.hlc,
            entity.hlc,
            type_id
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_sync_tombstone(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM sync_tombstones WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn ensure_usage_sequence_migrated(conn: &Connection, device_id: &str) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT usage_sequence_migration")
        .map_err(|e| e.to_string())?;
    match ensure_usage_sequence_migrated_inner(conn, device_id) {
        Ok(()) => {
            conn.execute_batch("RELEASE usage_sequence_migration")
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO usage_sequence_migration; RELEASE usage_sequence_migration",
            );
            Err(error)
        }
    }
}

fn ensure_usage_sequence_migrated_inner(conn: &Connection, device_id: &str) -> Result<(), String> {
    if get_sync_kv(conn, USAGE_SEQUENCE_MIGRATION_KEY)?.as_deref() == Some("1") {
        return Ok(());
    }

    let raw = get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(value).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default();
    let mut statement = conn
        .prepare(
            "SELECT entity_type, id FROM (
                SELECT 'usage_session' AS entity_type, id FROM usage_sessions
                UNION ALL
                SELECT 'usage_event', id FROM usage_events
                UNION ALL
                SELECT 'usage_day', id FROM usage_days
             )
             ORDER BY entity_type, id",
        )
        .map_err(|e| e.to_string())?;
    let refs = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(statement);

    let wall_time = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    let mut seq = 0u64;
    for (entity_type, entity_id) in refs {
        seq = seq.saturating_add(1);
        let hlc = HLC::new(wall_time.clone(), seq, device_id.to_string()).to_string();
        conn.execute(
            "INSERT OR REPLACE INTO usage_sync_versions(entity_type, entity_id, hlc)
             VALUES (?1, ?2, ?3)",
            params![entity_type, entity_id, hlc],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO usage_sync_log
                (device_id, seq, entity_type, entity_id, hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            params![device_id, seq as i64, entity_type, entity_id, hlc],
        )
        .map_err(|e| e.to_string())?;
        vector.remove(&entity_id);
    }
    conn.execute(
        "INSERT OR IGNORE INTO usage_sync_heads(device_id, max_seq) VALUES (?1, 0)",
        params![device_id],
    )
    .map_err(|e| e.to_string())?;
    let mut head = conn
        .query_row(
            "SELECT max_seq FROM usage_sync_heads WHERE device_id = ?1",
            params![device_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        .max(0) as u64;
    while conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM usage_sync_log WHERE device_id = ?1 AND seq = ?2
             )",
            params![device_id, head.saturating_add(1) as i64],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0
    {
        head = head.saturating_add(1);
    }
    conn.execute(
        "UPDATE usage_sync_heads SET max_seq = ?2 WHERE device_id = ?1",
        params![device_id, head as i64],
    )
    .map_err(|e| e.to_string())?;
    vector.insert(usage_cursor_key(device_id), head.to_string());
    set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|e| e.to_string())?,
    )?;
    set_sync_kv(conn, USAGE_SEQUENCE_MIGRATION_KEY, "1")
}

fn next_usage_sequence(conn: &Connection, device_id: &str) -> Result<u64, String> {
    conn.query_row(
        "INSERT INTO usage_sync_heads(device_id, max_seq) VALUES (?1, 1)
         ON CONFLICT(device_id) DO UPDATE SET max_seq = max_seq + 1
         RETURNING max_seq",
        params![device_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value.max(0) as u64)
    .map_err(|e| e.to_string())
}
