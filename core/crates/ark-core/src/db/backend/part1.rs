// ---------------------------------------------------------------------------
// Delete helpers (for StorageBackend)
// ---------------------------------------------------------------------------

pub fn delete_tag(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SqliteStorageBackend
//
// Wraps a shared rusqlite::Connection behind `Arc<Mutex<_>>` and implements
// the async `StorageBackend` trait expected by `sync_server::SyncServer`
// and `sync_client::SyncClient`. Blocking rusqlite work is wrapped in
// `tokio::task::spawn_blocking` so it doesn't stall the async runtime.
// ---------------------------------------------------------------------------

fn load_usage_sequence_page(
    conn: &Connection,
    remote_vector: &VersionVector,
    offset: usize,
    limit: usize,
) -> Result<Vec<SyncEntity>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let mut cursors = remote_vector
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix("@usage:")
                .and_then(|device_id| value.parse::<i64>().ok().map(|seq| (device_id, seq)))
        })
        .collect::<Vec<_>>();
    cursors.sort_unstable_by(|a, b| a.0.cmp(b.0));

    let mut sql = String::from(
        "SELECT device_id, seq, entity_type, entity_id
         FROM usage_sync_log WHERE seq > ",
    );
    let mut values = Vec::<rusqlite::types::Value>::new();
    if cursors.is_empty() {
        sql.push('0');
    } else {
        sql.push_str("CASE device_id ");
        for (device_id, seq) in cursors {
            sql.push_str("WHEN ? THEN ? ");
            values.push(device_id.to_string().into());
            values.push(seq.into());
        }
        sql.push_str("ELSE 0 END");
    }
    sql.push_str(" ORDER BY device_id, seq LIMIT ? OFFSET ?");
    values.push((limit as i64).into());
    values.push((offset as i64).into());

    let mut statement = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let refs = statement
        .query_map(params_from_iter(values), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(statement);

    fn to_data_map<T: serde::Serialize>(item: &T) -> serde_json::Map<String, Value> {
        match serde_json::to_value(item) {
            Ok(Value::Object(mut map)) => {
                map.remove("id");
                map
            }
            _ => serde_json::Map::new(),
        }
    }

    let mut entities = Vec::with_capacity(refs.len());
    for (origin_device_id, seq, entity_type, entity_id) in refs {
        let hlc = conn
            .query_row(
                "SELECT hlc FROM usage_sync_versions
                 WHERE entity_type = ?1 AND entity_id = ?2",
                params![entity_type, entity_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| HLC::now(&origin_device_id).to_string());
        let deleted = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sync_tombstones
                    WHERE entity_type = ?1 AND id = ?2
                 )",
                params![entity_type, entity_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            != 0;
        let data = if deleted {
            Some(serde_json::Map::new())
        } else {
            match entity_type.as_str() {
                "usage_session" => load_usage_session(conn, &entity_id)?
                    .as_ref()
                    .map(to_data_map),
                "usage_event" => load_usage_event(conn, &entity_id)?
                    .as_ref()
                    .map(to_data_map),
                "usage_day" => load_usage_day(conn, &entity_id)?.as_ref().map(to_data_map),
                _ => None,
            }
        };
        let Some(data) = data else {
            continue;
        };
        entities.push(SyncEntity {
            entity_type,
            id: entity_id,
            data,
            hlc,
            deleted: deleted.then_some(true),
            origin_device_id: Some(origin_device_id),
            origin_seq: Some(seq.max(0) as u64),
        });
    }
    Ok(entities)
}
