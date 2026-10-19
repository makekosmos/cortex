use ark_domain::types::SyncEntity;
use rusqlite::{params, Connection};
use serde_json::Value;

const LEGACY_INDEX: &str = "idx_sync_pending_awaited_type";
const VERSION_INDEX: &str = "idx_sync_pending_awaited_type_version";
const LEGACY_INDEX_SQL: &str =
    "CREATE INDEX idx_sync_pending_awaited_type ON sync_pending_objects(awaited_type_id)";
const VERSION_INDEX_SQL: &str = concat!(
    "CREATE INDEX idx_sync_pending_awaited_type_version ON sync_pending_objects(",
    "awaited_type_id,awaited_type_version)"
);

fn error(message: impl Into<String>) -> String {
    message.into()
}

fn table_columns(conn: &Connection) -> Result<Vec<(String, String, i64, i64)>, String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(sync_pending_objects)")
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([], |row| {
            Ok((row.get(1)?, row.get(2)?, row.get(3)?, row.get(5)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

fn exact_shape(columns: &[(String, String, i64, i64)], v3: bool) -> bool {
    let names = if v3 {
        vec![
            "id",
            "payload",
            "awaited_type_id",
            "awaited_type_version",
            "received_at",
        ]
    } else {
        let canonical = columns
            .iter()
            .map(|(name, _, _, _)| name.as_str())
            .collect::<Vec<_>>();
        if canonical
            == [
                "id",
                "payload",
                "awaited_type_id",
                "awaited_type_version",
                "received_at",
            ]
        {
            vec![
                "id",
                "payload",
                "awaited_type_id",
                "awaited_type_version",
                "received_at",
            ]
        } else {
            vec![
                "id",
                "payload",
                "awaited_type_id",
                "received_at",
                "awaited_type_version",
            ]
        }
    };
    if columns.len() != names.len() {
        return false;
    }
    columns
        .iter()
        .enumerate()
        .all(|(index, (name, ty, not_null, pk))| {
            let expected_pk = if v3 {
                [1, 0, 2, 3, 0][index]
            } else {
                [1, 0, 0, 0, 0][index]
            };
            name == names[index]
                && (index == 0 || *not_null == 1)
                && (*pk == expected_pk)
                && (index != 0 || ty.eq_ignore_ascii_case("TEXT"))
                && (index != 1 || ty.eq_ignore_ascii_case("TEXT"))
                && (index != 2 || ty.eq_ignore_ascii_case("TEXT"))
                && (index != 3 || ty.eq_ignore_ascii_case("TEXT"))
                && (index != 4 || ty.eq_ignore_ascii_case("TEXT"))
        })
}

pub fn validate_entity_tuple(
    entity: &SyncEntity,
    id: &str,
    awaited_type_id: &str,
    awaited_type_version: &str,
) -> Result<(), String> {
    if entity.id != id {
        return Err(error("pending payload id does not match row id"));
    }
    let payload_type_id = entity.data.get("typeId").and_then(Value::as_str);
    let payload_version = entity.data.get("typeVersion").and_then(Value::as_str);
    if payload_type_id != Some(awaited_type_id) || payload_version != Some(awaited_type_version) {
        return Err(error("pending payload type does not match awaited tuple"));
    }
    Ok(())
}

fn validate_row(row: &(String, String, String, String, String)) -> Result<(), String> {
    let entity: SyncEntity = serde_json::from_str(&row.1).map_err(|e| e.to_string())?;
    validate_entity_tuple(&entity, &row.0, &row.2, &row.3)
}

// Mirrors the `sync_pending_objects` column order in the SELECT below.
type PendingRow = (String, String, String, String, String);

fn rows(conn: &Connection) -> Result<Vec<PendingRow>, String> {
    let mut stmt = conn
        .prepare(concat!(
            "SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM ",
            "sync_pending_objects ORDER BY id,awaited_type_id,awaited_type_version"
        ))
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

fn indexes(conn: &Connection) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare(concat!(
            "SELECT name, sql FROM sqlite_master WHERE type='index' AND ",
            "tbl_name='sync_pending_objects' AND name IN (?1,?2) ORDER BY name"
        ))
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map(params![LEGACY_INDEX, VERSION_INDEX], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

fn validate_v3_state(conn: &Connection) -> Result<(), String> {
    for row in rows(conn)? {
        validate_row(&row)?;
    }
    let persisted = indexes(conn)?;
    let expected = vec![
        (LEGACY_INDEX.to_string(), LEGACY_INDEX_SQL.to_string()),
        (VERSION_INDEX.to_string(), VERSION_INDEX_SQL.to_string()),
    ];
    if persisted != expected {
        return Err(error("pending migration indexes mismatch"));
    }
    Ok(())
}

pub fn migrate_phase2_to_v3(conn: &Connection) -> Result<(), String> {
    let columns = table_columns(conn)?;
    if exact_shape(&columns, true) {
        return validate_v3_state(conn);
    }
    if !exact_shape(&columns, false) {
        return Err(error("unexpected sync_pending_objects schema"));
    }
    let snapshot = rows(conn)?;
    for row in &snapshot {
        validate_row(row)?;
    }
    conn.execute_batch("SAVEPOINT ark_phase3_pending")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        conn.execute_batch(concat!(
            "CREATE TABLE sync_pending_objects_v3 (id TEXT NOT NULL, payload TEXT NOT ",
            "NULL, awaited_type_id TEXT NOT NULL, awaited_type_version TEXT NOT NULL, ",
            "received_at TEXT NOT NULL, PRIMARY KEY(id, awaited_type_id, ",
            "awaited_type_version))"
        ))
        .map_err(|e| e.to_string())?;
        conn.execute(
            concat!(
                "INSERT INTO sync_pending_objects_v3 (id,payload,awaited_type_id,",
                "awaited_type_version,received_at) SELECT id,payload,awaited_type_id,",
                "awaited_type_version,received_at FROM sync_pending_objects"
            ),
            [],
        )
        .map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(concat!(
                "SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM ",
                "sync_pending_objects_v3 ORDER BY id,awaited_type_id,awaited_type_version"
            ))
            .map_err(|e| e.to_string())?;
        let copied_rows: Vec<(String, String, String, String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);
        if copied_rows != snapshot {
            return Err(error("pending migration snapshot mismatch"));
        }
        conn.execute_batch(concat!(
            "DROP TABLE sync_pending_objects; ALTER TABLE sync_pending_objects_v3 RENAME ",
            "TO sync_pending_objects; CREATE INDEX idx_sync_pending_awaited_type ON ",
            "sync_pending_objects(awaited_type_id); CREATE INDEX ",
            "idx_sync_pending_awaited_type_version ON sync_pending_objects(",
            "awaited_type_id,awaited_type_version)"
        ))
        .map_err(|e| e.to_string())?;
        if rows(conn)? != snapshot {
            return Err(error("pending migration post-snapshot mismatch"));
        }
        validate_v3_state(conn)?;
        Ok::<(), String>(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("RELEASE SAVEPOINT ark_phase3_pending")
            .map_err(|e| e.to_string()),
        Err(e) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_phase3_pending; RELEASE SAVEPOINT ark_phase3_pending",
            );
            Err(e)
        }
    }
}

pub fn is_v3_schema(connection: &Connection) -> Result<bool, String> {
    Ok(exact_shape(&table_columns(connection)?, true))
}

#[cfg(test)]
mod tests {
    use super::{is_v3_schema, migrate_phase2_to_v3};
    use ark_domain::types::SyncEntity;
    use rusqlite::Connection;
    use serde_json::{json, Map};

    fn legacy_pending_database() -> Connection {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(
                "CREATE TABLE sync_pending_objects (
                    id TEXT PRIMARY KEY,
                    payload TEXT NOT NULL,
                    awaited_type_id TEXT NOT NULL,
                    awaited_type_version TEXT NOT NULL,
                    received_at TEXT NOT NULL
                );
                CREATE INDEX idx_sync_pending_awaited_type
                    ON sync_pending_objects(awaited_type_id);
                CREATE INDEX idx_sync_pending_awaited_type_version
                    ON sync_pending_objects(awaited_type_id, awaited_type_version);",
            )
            .expect("create legacy pending schema");
        connection
    }

    fn entity(id: &str, type_id: &str, version: &str) -> SyncEntity {
        SyncEntity {
            entity_type: "object".into(),
            id: id.into(),
            data: Map::from_iter([
                ("id".into(), json!(id)),
                ("typeId".into(), json!(type_id)),
                ("typeVersion".into(), json!(version)),
            ]),
            hlc: "2026-10-19T00:00:00Z:1@device".into(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    #[test]
    fn migration_preserves_rows_and_is_idempotent() {
        let connection = legacy_pending_database();
        let payload = serde_json::to_string(&entity("object-1", "book", "1.0.0"))
            .expect("serialize pending entity");
        connection
            .execute(
                "INSERT INTO sync_pending_objects VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params!["object-1", payload, "book", "1.0.0", "received"],
            )
            .expect("insert legacy pending row");

        migrate_phase2_to_v3(&connection).expect("migrate legacy schema");
        migrate_phase2_to_v3(&connection).expect("retry completed migration");

        assert!(is_v3_schema(&connection).expect("inspect migrated schema"));
        let persisted: String = connection
            .query_row(
                "SELECT payload FROM sync_pending_objects WHERE id='object-1'
                 AND awaited_type_id='book' AND awaited_type_version='1.0.0'",
                [],
                |row| row.get(0),
            )
            .expect("read migrated row");
        assert_eq!(persisted, payload);
    }

    #[test]
    fn migration_rejects_inconsistent_payload_before_rewriting_schema() {
        let connection = legacy_pending_database();
        let payload = serde_json::to_string(&entity("object-1", "other", "1.0.0"))
            .expect("serialize mismatched entity");
        connection
            .execute(
                "INSERT INTO sync_pending_objects VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params!["object-1", payload, "book", "1.0.0", "received"],
            )
            .expect("insert inconsistent legacy row");

        assert!(migrate_phase2_to_v3(&connection).is_err());
        assert!(!is_v3_schema(&connection).expect("inspect original schema"));
        let row_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sync_pending_objects", [], |row| {
                row.get(0)
            })
            .expect("count original rows");
        assert_eq!(row_count, 1);
    }
}
