use std::cmp::Ordering;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::hlc::HLC;
use crate::types::SyncEntity;

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

fn validate_entity_tuple(
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
        let copied = conn.query_row(
            concat!(
                "SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM ",
                "sync_pending_objects_v3 ORDER BY id,awaited_type_id,awaited_type_version"
            ),
            [],
            |_| Ok(()),
        );
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
        let _ = copied;
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
