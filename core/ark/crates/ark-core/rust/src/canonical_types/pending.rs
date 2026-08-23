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
const VERSION_INDEX_SQL: &str = "CREATE INDEX idx_sync_pending_awaited_type_version ON sync_pending_objects(awaited_type_id,awaited_type_version)";

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

fn rows(conn: &Connection) -> Result<Vec<(String, String, String, String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM sync_pending_objects ORDER BY id,awaited_type_id,awaited_type_version")
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
        .prepare("SELECT name, sql FROM sqlite_master WHERE type='index' AND tbl_name='sync_pending_objects' AND name IN (?1,?2) ORDER BY name")
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
        conn.execute_batch("CREATE TABLE sync_pending_objects_v3 (id TEXT NOT NULL, payload TEXT NOT NULL, awaited_type_id TEXT NOT NULL, awaited_type_version TEXT NOT NULL, received_at TEXT NOT NULL, PRIMARY KEY(id, awaited_type_id, awaited_type_version))")
            .map_err(|e| e.to_string())?;
        conn.execute("INSERT INTO sync_pending_objects_v3 (id,payload,awaited_type_id,awaited_type_version,received_at) SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM sync_pending_objects", [])
            .map_err(|e| e.to_string())?;
        let copied = conn.query_row("SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM sync_pending_objects_v3 ORDER BY id,awaited_type_id,awaited_type_version", [], |_| Ok(()));
        let mut stmt = conn.prepare("SELECT id,payload,awaited_type_id,awaited_type_version,received_at FROM sync_pending_objects_v3 ORDER BY id,awaited_type_id,awaited_type_version").map_err(|e| e.to_string())?;
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
        conn.execute_batch("DROP TABLE sync_pending_objects; ALTER TABLE sync_pending_objects_v3 RENAME TO sync_pending_objects; CREATE INDEX idx_sync_pending_awaited_type ON sync_pending_objects(awaited_type_id); CREATE INDEX idx_sync_pending_awaited_type_version ON sync_pending_objects(awaited_type_id,awaited_type_version)")
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
