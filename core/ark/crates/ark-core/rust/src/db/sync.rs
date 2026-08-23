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
    conn.execute(
        "INSERT INTO sync_tombstones (id, entity_type, hlc, deleted_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            entity_type = excluded.entity_type,
            hlc = excluded.hlc,
            deleted_at = excluded.deleted_at
         WHERE excluded.hlc >= sync_tombstones.hlc",
        params![entity.id, entity.entity_type, entity.hlc, entity.hlc],
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

fn record_usage_sequence(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: &str,
    seq: u64,
    hlc: &str,
    deleted: bool,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO usage_sync_log
            (device_id, seq, entity_type, entity_id, hlc, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            device_id,
            seq as i64,
            entity_type,
            entity_id,
            hlc,
            deleted as i64
        ],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO usage_sync_heads(device_id, max_seq) VALUES (?1, ?2)
         ON CONFLICT(device_id) DO UPDATE SET max_seq = MAX(max_seq, excluded.max_seq)",
        params![device_id, seq as i64],
    )
    .map_err(|e| e.to_string())?;

    let raw = get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();
    let cursor_key = usage_cursor_key(device_id);
    let mut contiguous = vector
        .get(&cursor_key)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    while conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM usage_sync_log WHERE device_id = ?1 AND seq = ?2
             )",
            params![device_id, contiguous.saturating_add(1) as i64],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0
    {
        contiguous = contiguous.saturating_add(1);
    }
    vector.insert(cursor_key, contiguous.to_string());
    vector.remove(entity_id);
    set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|e| e.to_string())?,
    )
}

fn usage_origin_for_entity(
    conn: &Connection,
    entity: &SyncEntity,
) -> Result<(String, u64), String> {
    if let (Some(device_id), Some(seq)) = (&entity.origin_device_id, entity.origin_seq) {
        return Ok((device_id.clone(), seq));
    }
    if let Some(existing) = conn
        .query_row(
            "SELECT device_id, seq FROM usage_sync_log
             WHERE entity_type = ?1 AND entity_id = ?2 AND hlc = ?3
             LIMIT 1",
            params![entity.entity_type, entity.id, entity.hlc],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        return Ok((existing.0, existing.1.max(0) as u64));
    }
    let parsed = HLC::from_string(&entity.hlc);
    let source_device_id = if parsed.device_id.is_empty() {
        "legacy-usage".to_string()
    } else {
        parsed.device_id
    };
    let device_id = format!("legacy:{source_device_id}");
    let seq = next_usage_sequence(conn, &device_id)?;
    Ok((device_id, seq))
}

fn usage_entity_is_newer(conn: &Connection, entity: &SyncEntity) -> Result<bool, String> {
    let local = conn
        .query_row(
            "SELECT hlc FROM usage_sync_versions
             WHERE entity_type = ?1 AND entity_id = ?2",
            params![entity.entity_type, entity.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(local
        .as_deref()
        .is_none_or(|hlc| HLC::is_newer(&entity.hlc, hlc)))
}

fn finish_usage_entity_sync(conn: &Connection, entity: &SyncEntity) -> Result<(), String> {
    conn.execute(
        "INSERT INTO usage_sync_versions(entity_type, entity_id, hlc)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(entity_type, entity_id) DO UPDATE SET hlc = excluded.hlc
         WHERE excluded.hlc > usage_sync_versions.hlc",
        params![entity.entity_type, entity.id, entity.hlc],
    )
    .map_err(|e| e.to_string())?;
    let (device_id, seq) = usage_origin_for_entity(conn, entity)?;
    record_usage_sequence(
        conn,
        &entity.entity_type,
        &entity.id,
        &device_id,
        seq,
        &entity.hlc,
        entity.deleted == Some(true),
    )
}

pub fn bump_sync_version_vector(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: &str,
    deleted: bool,
) -> Result<String, String> {
    if is_sequenced_usage_entity(entity_type) {
        ensure_usage_sequence_migrated(conn, device_id)?;
        let seq = next_usage_sequence(conn, device_id)?;
        let hlc = HLC::new(
            Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            seq,
            device_id.to_string(),
        )
        .to_string();
        conn.execute(
            "INSERT INTO usage_sync_versions(entity_type, entity_id, hlc)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(entity_type, entity_id) DO UPDATE SET hlc = excluded.hlc",
            params![entity_type, entity_id, hlc],
        )
        .map_err(|e| e.to_string())?;
        record_usage_sequence(conn, entity_type, entity_id, device_id, seq, &hlc, deleted)?;
        return Ok(hlc);
    }

    let raw = get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(value).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default();

    let now = HLC::now(device_id);
    let next = match vector.get(entity_id) {
        Some(existing) => {
            let previous = HLC::from_string(existing);
            if HLC::compare(&now, &previous).is_gt() {
                now
            } else {
                HLC::new(
                    previous.wall_time,
                    previous.counter.saturating_add(1),
                    device_id.to_string(),
                )
            }
        }
        None => now,
    };
    let hlc = next.to_string();
    vector.insert(entity_id.to_string(), hlc.clone());
    set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|e| e.to_string())?,
    )?;
    Ok(hlc)
}

pub fn record_sync_tombstone(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    hlc: &str,
) -> Result<(), String> {
    let entity = SyncEntity {
        entity_type: entity_type.to_string(),
        id: entity_id.to_string(),
        data: serde_json::Map::new(),
        hlc: hlc.to_string(),
        deleted: Some(true),
        origin_device_id: None,
        origin_seq: None,
    };
    upsert_sync_tombstone(conn, &entity)
}

fn load_sync_tombstones(conn: &Connection) -> Result<Vec<SyncEntity>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, entity_type, hlc
             FROM sync_tombstones
             ORDER BY hlc ASC, id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SyncEntity {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                hlc: row.get(2)?,
                data: serde_json::Map::new(),
                deleted: Some(true),
                origin_device_id: None,
                origin_seq: None,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

