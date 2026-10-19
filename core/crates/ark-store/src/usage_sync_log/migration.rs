use ark_domain::hlc::HLC;
use ark_domain::types::VersionVector;
use chrono::Utc;
use rusqlite::{params, Connection};

use crate::VERSION_VECTOR_KEY;

pub const USAGE_SEQUENCE_MIGRATION_KEY: &str = "usage_sync.sequence_v1";

/// Backfill the usage sequence log once, within the caller's transaction.
pub fn ensure_usage_sequence_migrated(conn: &Connection, device_id: &str) -> Result<(), String> {
    conn.execute_batch("SAVEPOINT usage_sequence_migration")
        .map_err(|error| error.to_string())?;
    match ensure_usage_sequence_migrated_inner(conn, device_id) {
        Ok(()) => {
            conn.execute_batch("RELEASE usage_sequence_migration")
                .map_err(|error| error.to_string())?;
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
    if crate::sync_kv::get_sync_kv(conn, USAGE_SEQUENCE_MIGRATION_KEY)?.as_deref() == Some("1") {
        return Ok(());
    }

    let raw = crate::sync_kv::get_sync_kv(conn, VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(value).map_err(|error| error.to_string()))
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
        .map_err(|error| error.to_string())?;
    let refs = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
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
        .map_err(|error| error.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO usage_sync_log
                (device_id, seq, entity_type, entity_id, hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            params![device_id, seq as i64, entity_type, entity_id, hlc],
        )
        .map_err(|error| error.to_string())?;
        vector.remove(&entity_id);
    }
    conn.execute(
        "INSERT OR IGNORE INTO usage_sync_heads(device_id, max_seq) VALUES (?1, 0)",
        params![device_id],
    )
    .map_err(|error| error.to_string())?;
    let mut head = conn
        .query_row(
            "SELECT max_seq FROM usage_sync_heads WHERE device_id = ?1",
            params![device_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?
        .max(0) as u64;
    while conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM usage_sync_log WHERE device_id = ?1 AND seq = ?2
             )",
            params![device_id, head.saturating_add(1) as i64],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?
        != 0
    {
        head = head.saturating_add(1);
    }
    conn.execute(
        "UPDATE usage_sync_heads SET max_seq = ?2 WHERE device_id = ?1",
        params![device_id, head as i64],
    )
    .map_err(|error| error.to_string())?;
    vector.insert(format!("@usage:{device_id}"), head.to_string());
    crate::sync_kv::set_sync_kv(
        conn,
        VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|error| error.to_string())?,
    )?;
    crate::sync_kv::set_sync_kv(conn, USAGE_SEQUENCE_MIGRATION_KEY, "1")
}
