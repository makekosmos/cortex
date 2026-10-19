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
    if ark_store::sync_kv::get_sync_kv(conn, USAGE_SEQUENCE_MIGRATION_KEY)?.as_deref() == Some("1")
    {
        return Ok(());
    }

    let raw = ark_store::sync_kv::get_sync_kv(conn, VERSION_VECTOR_KEY)?;
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
    ark_store::sync_kv::set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|e| e.to_string())?,
    )?;
    ark_store::sync_kv::set_sync_kv(conn, USAGE_SEQUENCE_MIGRATION_KEY, "1")
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
        let seq = ark_store::usage_sync_log::next_usage_sequence(conn, device_id)?;
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
        ark_store::usage_sync_log::record_usage_sequence(
            conn,
            entity_type,
            entity_id,
            device_id,
            seq,
            &hlc,
            deleted,
        )?;
        ark_store::committed_feed::append_committed_change(
            conn,
            entity_type,
            entity_id,
            &hlc,
            deleted,
            Some(device_id),
            Some(seq),
        )?;
        return Ok(hlc);
    }

    let raw = ark_store::sync_kv::get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(value).map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or_default();

    let mut previous = vector.get(entity_id).cloned();
    if entity_type == "object" {
        if let Some(candidate) = ark_store::sync_versions::object_revision(conn, entity_id)? {
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
    ark_store::sync_kv::set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|e| e.to_string())?,
    )?;
    ark_store::committed_feed::append_committed_change(
        conn,
        entity_type,
        entity_id,
        &hlc,
        deleted,
        Some(device_id),
        None,
    )?;
    Ok(hlc)
}
