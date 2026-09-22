
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

pub fn get_object_revision(conn: &Connection, entity_id: &str) -> Result<Option<String>, String> {
    let vector_revision = get_sync_kv(conn, VERSION_VECTOR_KEY)?
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str::<VersionVector>(&value).map_err(|error| error.to_string()))
        .transpose()?
        .and_then(|vector| vector.get(entity_id).cloned());
    let object_revision = conn
        .query_row(
            "SELECT hlc FROM object_sync_versions WHERE object_id = ?1",
            [entity_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let tombstone_revision = conn
        .query_row(
            "SELECT hlc FROM sync_tombstones WHERE id = ?1 AND entity_type = 'object'",
            [entity_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok([vector_revision, object_revision, tombstone_revision]
        .into_iter()
        .flatten()
        .max_by(|left, right| HLC::compare(&HLC::from_string(left), &HLC::from_string(right))))
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

    let mut previous = vector.get(entity_id).cloned();
    if entity_type == "object" {
        if let Some(candidate) = get_object_revision(conn, entity_id)? {
            previous = Some(candidate);
        }
    }

    let now = HLC::now(device_id);
    let next = match previous {
        Some(existing) => {
            let previous = HLC::from_string(&existing);
            if HLC::compare(&now, &previous).is_gt() {
                now
            } else {
                if previous.counter == u64::MAX {
                    return Err("hlc_counter_overflow".to_string());
                }
                HLC::new(
                    previous.wall_time,
                    previous.counter + 1,
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
