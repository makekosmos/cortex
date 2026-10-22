use ark_application::sync_storage::{PolicyBoundReadError, PolicyBoundSnapshotPage};
use ark_domain::sync_policy::PolicyIdentity;
use ark_domain::types::{SyncEntity, VersionVector};
use rusqlite::Connection;

/// Reads one page under the same SQLite snapshot as the peer policy check.
/// A stale policy rolls back the read savepoint before returning to the caller.
pub fn read_policy_bound_page(
    conn: &Connection,
    peer_device_id: &str,
    expected_identity: &PolicyIdentity,
    vector: &VersionVector,
    device_id: &str,
    offset: usize,
    limit: usize,
) -> Result<PolicyBoundSnapshotPage, PolicyBoundReadError> {
    conn.execute_batch("SAVEPOINT sync_policy_snapshot_page")
        .map_err(|error| PolicyBoundReadError::Storage(error.to_string()))?;
    let result = read_page(conn, peer_device_id, vector, device_id, offset, limit);
    match result {
        Ok((identity, entities)) if &identity == expected_identity => {
            conn.execute_batch("RELEASE sync_policy_snapshot_page")
                .map_err(|error| PolicyBoundReadError::Storage(error.to_string()))?;
            Ok(PolicyBoundSnapshotPage { identity, entities })
        }
        Ok(_) => {
            rollback_read(conn)?;
            Err(PolicyBoundReadError::StalePolicy)
        }
        Err(error) => {
            rollback_read(conn)?;
            Err(PolicyBoundReadError::Storage(error))
        }
    }
}

fn read_page(
    conn: &Connection,
    peer_device_id: &str,
    vector: &VersionVector,
    device_id: &str,
    offset: usize,
    limit: usize,
) -> Result<(PolicyIdentity, Vec<SyncEntity>), String> {
    let policy = crate::data_platform::load_accepted_peer_sync_policy(conn, peer_device_id)?;
    let identity = policy.as_ref().map_or_else(
        ark_application::sync_policy::default_sync_policy_identity,
        |policy| policy.identity(),
    );
    let entities = match policy.filter(|policy| policy.active) {
        Some(policy) => {
            let all = crate::sync_entities::collect_entities(conn, vector, device_id)?;
            crate::data_platform::filter_entities_for_profile(&policy.selective_profile(), &all)
                .into_iter()
                .skip(offset)
                .take(limit)
                .collect()
        }
        None => {
            crate::sync_entities::collect_entities_page(conn, vector, device_id, offset, limit)?
        }
    };
    Ok((identity, entities))
}

fn rollback_read(conn: &Connection) -> Result<(), PolicyBoundReadError> {
    conn.execute_batch("ROLLBACK TO sync_policy_snapshot_page; RELEASE sync_policy_snapshot_page")
        .map_err(|error| PolicyBoundReadError::Storage(error.to_string()))
}
