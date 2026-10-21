use ark_domain::hlc::HLC;
use ark_domain::types::SyncEntity;
use chrono::Utc;
use rusqlite::Connection;
use serde_json::Value;
use std::cmp::Ordering;

pub use ark_application::sync_storage::ReplayNotice;

pub fn insert_pending_object(
    connection: &Connection,
    entity: &SyncEntity,
    awaited_type_id: &str,
) -> Result<(), String> {
    let awaited_type_version = entity
        .data
        .get("typeVersion")
        .and_then(Value::as_str)
        .unwrap_or(ark_domain::canonical_type::registry::LEGACY_VERSION);
    let schema = crate::pending_objects::schema(connection)?;
    let is_versioned = schema == crate::pending_objects::PendingObjectSchema::Versioned;
    if is_versioned {
        crate::pending_objects::validate_entity_tuple(
            entity,
            &entity.id,
            awaited_type_id,
            awaited_type_version,
        )?;
    }
    let payload = serde_json::to_string(entity).map_err(|error| error.to_string())?;
    let existing = crate::pending_objects::load_payload(
        connection,
        schema,
        &entity.id,
        awaited_type_id,
        awaited_type_version,
    )?;
    if let Some(existing) = existing {
        let old: SyncEntity = serde_json::from_str(&existing).map_err(|error| error.to_string())?;
        if HLC::compare_str(&entity.hlc, &old.hlc) != Ordering::Greater {
            return Ok(());
        }
    }
    crate::pending_objects::upsert(
        connection,
        schema,
        &crate::pending_objects::PendingObjectRow {
            id: entity.id.clone(),
            payload,
            awaited_type_id: awaited_type_id.to_owned(),
            awaited_type_version: awaited_type_version.to_owned(),
            received_at: Utc::now().to_rfc3339(),
        },
    )
}

pub fn replay_pending_for_type(
    connection: &Connection,
    type_id: &str,
    type_version: &str,
) -> Result<Vec<ReplayNotice>, String> {
    if !crate::pending_objects::table_exists(connection)? {
        return Ok(Vec::new());
    }
    let schema = crate::pending_objects::schema(connection)?;
    let is_versioned = schema == crate::pending_objects::PendingObjectSchema::Versioned;
    let pending = crate::pending_objects::load_for_type(connection, type_id, type_version)?;
    connection
        .execute_batch("SAVEPOINT ark_replay_pending")
        .map_err(|error| error.to_string())?;
    let result = (|| {
        for row in &pending {
            let entity: SyncEntity =
                serde_json::from_str(&row.payload).map_err(|error| error.to_string())?;
            if is_versioned {
                crate::pending_objects::validate_entity_tuple(
                    &entity,
                    &row.id,
                    &row.awaited_type_id,
                    &row.awaited_type_version,
                )?;
            }
            crate::sync_entities::apply_canonical_object_entity(connection, &entity)?;
            crate::pending_objects::delete(
                connection,
                &row.id,
                &row.awaited_type_id,
                &row.awaited_type_version,
            )?;
        }
        Ok::<_, String>(())
    })();
    match result {
        Ok(()) => connection
            .execute_batch("RELEASE SAVEPOINT ark_replay_pending")
            .map_err(|error| error.to_string())?,
        Err(error) => {
            let _ = connection.execute_batch(
                "ROLLBACK TO SAVEPOINT ark_replay_pending; RELEASE SAVEPOINT ark_replay_pending",
            );
            return Err(error);
        }
    }
    Ok(pending
        .into_iter()
        .map(|row| ReplayNotice {
            entity_id: row.id,
            type_id: type_id.to_owned(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{insert_pending_object, replay_pending_for_type};
    use crate::schema::CREATE_TABLES;
    use ark_domain::types::SyncEntity;
    use rusqlite::Connection;
    use serde_json::{json, Map};

    #[test]
    fn pending_entity_is_applied_and_removed_when_its_type_appears() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        connection
            .execute_batch(CREATE_TABLES)
            .expect("create base schema");
        connection
            .execute_batch(
                "CREATE TABLE object_sync_versions (
                    object_id TEXT PRIMARY KEY,
                    hlc TEXT NOT NULL,
                    deleted INTEGER NOT NULL DEFAULT 0
                );
                CREATE TABLE sync_pending_objects (
                    id TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    awaited_type_id TEXT NOT NULL,
                    awaited_type_version TEXT NOT NULL,
                    received_at TEXT NOT NULL,
                    PRIMARY KEY(id, awaited_type_id, awaited_type_version)
                );",
            )
            .expect("create sync tables");
        connection
            .execute(
                concat!(
                    "INSERT INTO object_types(id,name,schema_json,ui_schema_json,",
                    "created_at,updated_at) VALUES('future','Future','{}','{}','now','now')"
                ),
                [],
            )
            .expect("insert object type");

        let entity = SyncEntity {
            entity_type: "object".into(),
            id: "future-object".into(),
            data: Map::from_iter([
                ("typeId".into(), json!("future")),
                ("typeVersion".into(), json!("1.0.0")),
                ("title".into(), json!("Awaiting type")),
                ("props".into(), json!({})),
                ("createdAt".into(), json!("2026-10-20T10:00:00.000Z")),
                ("updatedAt".into(), json!("2026-10-20T10:00:00.000Z")),
            ]),
            hlc: "2026-10-20T10:00:00.000Z-0000000000@device".into(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };
        insert_pending_object(&connection, &entity, "future").expect("store pending object");

        assert_eq!(
            replay_pending_for_type(&connection, "future", "1.0.0").expect("replay pending object"),
            vec![ark_application::sync_storage::ReplayNotice {
                entity_id: "future-object".into(),
                type_id: "future".into(),
            }]
        );
        assert_eq!(
            crate::pending_objects::count_for_type(&connection, "future")
                .expect("count pending objects"),
            0
        );
        let title: String = connection
            .query_row(
                "SELECT title FROM objects WHERE id='future-object'",
                [],
                |row| row.get(0),
            )
            .expect("read applied object");
        assert_eq!(title, "Awaiting type");
    }
}
