use crate::hlc::HLC;
use crate::types::SyncEntity;
use chrono::Utc;
use rusqlite::Connection;
use serde_json::Value;
use std::cmp::Ordering;

pub use ark_application::sync_storage::ReplayNotice;

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
    let schema = ark_store::pending_objects::schema(conn)?;
    let is_v3 = schema == ark_store::pending_objects::PendingObjectSchema::Versioned;
    if is_v3 {
        ark_store::pending_objects::validate_entity_tuple(
            entity,
            &entity.id,
            awaited_type_id,
            awaited_type_version,
        )?;
    }
    let payload = serde_json::to_string(entity).map_err(|e| e.to_string())?;
    let existing = ark_store::pending_objects::load_payload(
        conn,
        schema,
        &entity.id,
        awaited_type_id,
        awaited_type_version,
    )?;
    if let Some(existing) = existing {
        let old: SyncEntity = serde_json::from_str(&existing).map_err(|e| e.to_string())?;
        if HLC::compare_str(&entity.hlc, &old.hlc) != Ordering::Greater {
            return Ok(());
        }
    }
    ark_store::pending_objects::upsert(
        conn,
        schema,
        &ark_store::pending_objects::PendingObjectRow {
            id: entity.id.clone(),
            payload,
            awaited_type_id: awaited_type_id.to_owned(),
            awaited_type_version: awaited_type_version.to_owned(),
            received_at: Utc::now().to_rfc3339(),
        },
    )
}

pub fn replay_pending_for_type(
    conn: &Connection,
    type_id: &str,
    type_version: &str,
) -> Result<Vec<ReplayNotice>, String> {
    let exists = ark_store::pending_objects::table_exists(conn)?;
    if !exists {
        return Ok(Vec::new());
    }
    let schema = ark_store::pending_objects::schema(conn)?;
    let is_v3 = schema == ark_store::pending_objects::PendingObjectSchema::Versioned;
    let pending = ark_store::pending_objects::load_for_type(conn, type_id, type_version)?;
    conn.execute_batch("SAVEPOINT ark_replay_pending")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        for row in &pending {
            let entity: SyncEntity =
                serde_json::from_str(&row.payload).map_err(|e| e.to_string())?;
            if is_v3 {
                ark_store::pending_objects::validate_entity_tuple(
                    &entity,
                    &row.id,
                    &row.awaited_type_id,
                    &row.awaited_type_version,
                )?;
            }
            crate::canonical_types::facades::apply_canonical_object_entity(conn, &entity)?;
            ark_store::pending_objects::delete(
                conn,
                &row.id,
                &row.awaited_type_id,
                &row.awaited_type_version,
            )?;
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
    Ok(pending
        .into_iter()
        .map(|row| ReplayNotice {
            entity_id: row.id,
            type_id: type_id.to_owned(),
        })
        .collect())
}

pub fn count_pending_for_type(conn: &Connection, type_id: &str) -> Result<i64, String> {
    ark_store::pending_objects::count_for_type(conn, type_id)
}
