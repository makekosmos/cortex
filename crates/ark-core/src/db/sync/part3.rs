
pub fn record_sync_tombstone(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    hlc: &str,
) -> Result<(), String> {
    record_sync_tombstone_with_type(conn, entity_type, entity_id, None, hlc)
}

pub fn record_sync_tombstone_with_type(
    conn: &Connection,
    entity_type: &str,
    entity_id: &str,
    type_id: Option<&str>,
    hlc: &str,
) -> Result<(), String> {
    let mut data = serde_json::Map::new();
    if let Some(type_id) = type_id {
        data.insert("typeId".into(), type_id.into());
    }
    let entity = SyncEntity {
        entity_type: entity_type.to_string(),
        id: entity_id.to_string(),
        data,
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
            "SELECT id, entity_type, hlc, type_id
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
                data: row
                    .get::<_, Option<String>>(3)?
                    .map(|type_id| [("typeId".into(), type_id.into())].into_iter().collect())
                    .unwrap_or_default(),
                deleted: Some(true),
                origin_device_id: None,
                origin_seq: None,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}
