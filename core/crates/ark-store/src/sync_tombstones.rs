use ark_domain::types::SyncEntity;
use rusqlite::{params, Connection};

pub fn upsert(connection: &Connection, entity: &SyncEntity) -> Result<(), String> {
    let type_id = entity.data.get("typeId").and_then(|value| value.as_str());
    connection
        .execute(
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
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn delete(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM sync_tombstones WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn record(
    connection: &Connection,
    entity_type: &str,
    entity_id: &str,
    type_id: Option<&str>,
    hlc: &str,
) -> Result<(), String> {
    let data = type_id
        .map(|type_id| [("typeId".into(), type_id.into())].into_iter().collect())
        .unwrap_or_default();
    upsert(
        connection,
        &SyncEntity {
            entity_type: entity_type.into(),
            id: entity_id.into(),
            data,
            hlc: hlc.into(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        },
    )
}

pub fn load_all(connection: &Connection) -> Result<Vec<SyncEntity>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, entity_type, hlc, type_id
             FROM sync_tombstones
             ORDER BY hlc ASC, id ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
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
        .map_err(|error| error.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{delete, load_all, record, upsert};
    use ark_domain::types::SyncEntity;
    use rusqlite::Connection;
    use serde_json::Map;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(
                "CREATE TABLE sync_tombstones (
                    id TEXT PRIMARY KEY,
                    entity_type TEXT NOT NULL,
                    hlc TEXT NOT NULL,
                    deleted_at TEXT NOT NULL,
                    type_id TEXT
                );",
            )
            .expect("tombstone table creates");
        connection
    }

    fn entity(id: &str, hlc: &str, type_id: Option<&str>) -> SyncEntity {
        let mut data = Map::new();
        if let Some(type_id) = type_id {
            data.insert("typeId".into(), type_id.into());
        }
        SyncEntity {
            entity_type: "object".into(),
            id: id.into(),
            data,
            hlc: hlc.into(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        }
    }

    #[test]
    fn tombstone_updates_are_monotonic_and_keep_known_type_identity() {
        let connection = connection();
        upsert(
            &connection,
            &entity(
                "object-1",
                "2026-10-01T00:00:00.000Z:000001:a",
                Some("note"),
            ),
        )
        .expect("initial tombstone persists");
        upsert(
            &connection,
            &entity("object-1", "2026-10-02T00:00:00.000Z:000001:b", None),
        )
        .expect("newer tombstone persists");
        upsert(
            &connection,
            &entity("object-1", "2026-10-01T00:00:00.000Z:000002:c", Some("old")),
        )
        .expect("older tombstone is ignored");

        let tombstones = load_all(&connection).expect("tombstones load");
        assert_eq!(
            tombstones,
            vec![entity(
                "object-1",
                "2026-10-02T00:00:00.000Z:000001:b",
                Some("note"),
            )]
        );
        record(
            &connection,
            "object",
            "object-2",
            Some("book"),
            "2026-10-02T00:00:00.000Z:000002:b",
        )
        .expect("typed tombstone records");
        delete(&connection, "object-1").expect("tombstone deletes");
        assert_eq!(
            load_all(&connection).expect("remaining tombstones load"),
            vec![entity(
                "object-2",
                "2026-10-02T00:00:00.000Z:000002:b",
                Some("book"),
            )]
        );
    }
}
