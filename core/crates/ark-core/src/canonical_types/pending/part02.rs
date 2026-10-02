
pub fn insert_pending_object(
    conn: &Connection,
    entity: &SyncEntity,
    awaited_type_id: &str,
) -> Result<(), String> {
    let awaited_type_version = entity
        .data
        .get("typeVersion")
        .and_then(Value::as_str)
        .unwrap_or("0.0.0-legacy");
    let is_v3 = exact_shape(&table_columns(conn)?, true);
    if is_v3 {
        validate_entity_tuple(entity, &entity.id, awaited_type_id, awaited_type_version)?;
    }
    let payload = serde_json::to_string(entity).map_err(|e| e.to_string())?;
    let existing_sql = if is_v3 {
        "SELECT payload FROM sync_pending_objects WHERE id=?1 AND awaited_type_id=?2 AND awaited_type_version=?3"
    } else {
        "SELECT payload FROM sync_pending_objects WHERE id=?1"
    };
    let existing: Option<String> = if is_v3 {
        conn.query_row(
            existing_sql,
            params![entity.id, awaited_type_id, awaited_type_version],
            |row| row.get(0),
        )
        .optional()
    } else {
        conn.query_row(existing_sql, params![entity.id], |row| row.get(0))
            .optional()
    }
    .map_err(|e| e.to_string())?;
    if let Some(existing) = existing {
        let old: SyncEntity = serde_json::from_str(&existing).map_err(|e| e.to_string())?;
        if HLC::compare_str(&entity.hlc, &old.hlc) != Ordering::Greater {
            return Ok(());
        }
    }
    let sql = if is_v3 {
        "INSERT INTO sync_pending_objects (id,payload,awaited_type_id,awaited_type_version,received_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id,awaited_type_id,awaited_type_version) DO UPDATE SET payload=excluded.payload,received_at=excluded.received_at"
    } else {
        "INSERT INTO sync_pending_objects (id,payload,awaited_type_id,awaited_type_version,received_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,awaited_type_id=excluded.awaited_type_id,awaited_type_version=excluded.awaited_type_version,received_at=excluded.received_at"
    };
    conn.execute(
        sql,
        params![
            entity.id,
            payload,
            awaited_type_id,
            awaited_type_version,
            Utc::now().to_rfc3339()
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn replay_pending_for_type(
    conn: &Connection,
    type_id: &str,
    type_version: &str,
) -> Result<usize, String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='sync_pending_objects')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !exists {
        return Ok(0);
    }
    let is_v3 = exact_shape(&table_columns(conn)?, true);
    let mut stmt = conn.prepare("SELECT id,payload,awaited_type_id,awaited_type_version FROM sync_pending_objects WHERE awaited_type_id=?1 AND awaited_type_version=?2 ORDER BY id").map_err(|e| e.to_string())?;
    let pending: Vec<(String, String, String, String)> = stmt
        .query_map(params![type_id, type_version], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    conn.execute_batch("SAVEPOINT ark_replay_pending")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        for (id, payload, awaited_id, awaited_version) in &pending {
            let entity: SyncEntity = serde_json::from_str(payload).map_err(|e| e.to_string())?;
            if is_v3 {
                validate_entity_tuple(&entity, id, awaited_id, awaited_version)?;
            }
            crate::canonical_types::facades::apply_canonical_object_entity(conn, &entity)?;
            conn.execute("DELETE FROM sync_pending_objects WHERE id=?1 AND awaited_type_id=?2 AND awaited_type_version=?3", params![id,awaited_id,awaited_version]).map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT ark_replay_pending")
            .map_err(|e| e.to_string())?,
        Err(e) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_replay_pending; RELEASE SAVEPOINT ark_replay_pending",
            );
            return Err(e);
        }
    }
    for (id, _, _, _) in &pending {
        crate::events::emit_event(
            json!({"event":"sync_replay","entity_type":"object","entity_id":id,"type_id":type_id}),
        );
    }
    Ok(pending.len())
}

pub fn count_pending_for_type(conn: &Connection, type_id: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM sync_pending_objects WHERE awaited_type_id=?1",
        params![type_id],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}
