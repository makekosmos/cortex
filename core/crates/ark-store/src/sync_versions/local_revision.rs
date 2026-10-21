use ark_domain::hlc::HLC;
use ark_domain::types::{is_sequenced_usage_entity, VersionVector};
use chrono::Utc;
use rusqlite::Connection;

use super::object_revision;

/// Persist one local sync revision inside the caller's existing transaction.
/// HLC selection, version-vector state, usage sequence metadata, and the
/// committed-feed row form one storage invariant and must not be split across
/// a Core façade and independent Store calls.
pub fn bump_sync_version_vector(
    connection: &Connection,
    entity_type: &str,
    entity_id: &str,
    device_id: &str,
    deleted: bool,
) -> Result<String, String> {
    if is_sequenced_usage_entity(entity_type) {
        crate::usage_sync_log::ensure_usage_sequence_migrated(connection, device_id)?;
        let sequence = crate::usage_sync_log::next_usage_sequence(connection, device_id)?;
        let revision = HLC::new(
            Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            sequence,
            device_id.to_string(),
        )
        .to_string();
        connection
            .execute(
                "INSERT INTO usage_sync_versions(entity_type, entity_id, hlc)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(entity_type, entity_id) DO UPDATE SET hlc = excluded.hlc",
                rusqlite::params![entity_type, entity_id, revision],
            )
            .map_err(|error| error.to_string())?;
        crate::usage_sync_log::record_usage_sequence(
            connection,
            entity_type,
            entity_id,
            device_id,
            sequence,
            &revision,
            deleted,
        )?;
        return Ok(revision);
    }

    let raw = crate::sync_kv::get_sync_kv(connection, crate::VERSION_VECTOR_KEY)?;
    let mut vector: VersionVector = raw
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| serde_json::from_str(value).map_err(|error| error.to_string()))
        .transpose()?
        .unwrap_or_default();
    let mut previous = vector.get(entity_id).cloned();
    if entity_type == "object" {
        if let Some(candidate) = object_revision(connection, entity_id)? {
            previous = Some(candidate);
        }
    }

    let now = HLC::now(device_id);
    let next = match previous {
        Some(existing) => {
            let previous = HLC::from_string(&existing);
            if HLC::compare(&now, &previous).is_gt() {
                now
            } else {
                if previous.counter == u64::MAX {
                    return Err("hlc_counter_overflow".to_string());
                }
                HLC::new(
                    previous.wall_time,
                    previous.counter + 1,
                    device_id.to_string(),
                )
            }
        }
        None => now,
    };
    let revision = next.to_string();
    vector.insert(entity_id.to_string(), revision.clone());
    crate::sync_kv::set_sync_kv(
        connection,
        crate::VERSION_VECTOR_KEY,
        &serde_json::to_string(&vector).map_err(|error| error.to_string())?,
    )?;
    crate::committed_feed::append_committed_change(
        connection,
        entity_type,
        entity_id,
        &revision,
        deleted,
        Some(device_id),
        None,
    )?;
    if entity_type == "object" && !deleted {
        connection
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted)
                 VALUES (?1, ?2, 0)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = 0
                 WHERE excluded.hlc > object_sync_versions.hlc",
                rusqlite::params![entity_id, revision],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute("DELETE FROM sync_tombstones WHERE id = ?1", [entity_id])
            .map_err(|error| error.to_string())?;
    }
    Ok(revision)
}
