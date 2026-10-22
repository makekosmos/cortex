use super::{storage_error, TaskMergeStoreError};
use ark_domain::sync_policy::PolicyIdentity;
use rusqlite::{Connection, OptionalExtension, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMergeOutboundProgress {
    pub identity: PolicyIdentity,
    pub acknowledged_through: i64,
    pub in_flight: Option<TaskMergeInFlightBatch>,
    pub last_acknowledged: Option<TaskMergeAcknowledgedBatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMergeInFlightBatch {
    pub batch_id: String,
    pub after: i64,
    pub through: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMergeAcknowledgedBatch {
    pub batch_id: String,
    pub through: i64,
}

pub fn load_outbound_progress(
    connection: &Connection,
    peer_device_id: &str,
) -> Result<Option<TaskMergeOutboundProgress>, TaskMergeStoreError> {
    let stored = connection
        .query_row(
            "SELECT policy_revision, policy_digest, policy_active, acknowledged_through,
                    in_flight_batch_id, in_flight_after, in_flight_through,
                    last_acknowledged_batch_id, last_acknowledged_through
             FROM task_merge_outbound_progress WHERE peer_device_id = ?1",
            [peer_device_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                ))
            },
        )
        .optional()
        .map_err(storage_error)?;
    let Some((
        revision,
        digest,
        active,
        acknowledged_through,
        batch_id,
        after,
        through,
        last_id,
        last_through,
    )) = stored
    else {
        return Ok(None);
    };
    let in_flight = match (batch_id, after, through) {
        (None, None, None) => None,
        (Some(batch_id), Some(after), Some(through)) => Some(TaskMergeInFlightBatch {
            batch_id,
            after,
            through,
        }),
        _ => {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_progress_corrupt".into(),
            ));
        }
    };
    let last_acknowledged = match (last_id, last_through) {
        (None, None) => None,
        (Some(batch_id), Some(through)) => Some(TaskMergeAcknowledgedBatch { batch_id, through }),
        _ => {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_progress_corrupt".into(),
            ));
        }
    };
    Ok(Some(TaskMergeOutboundProgress {
        identity: PolicyIdentity {
            revision,
            digest,
            active,
        },
        acknowledged_through,
        in_flight,
        last_acknowledged,
    }))
}

pub fn create_outbound_progress(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
) -> Result<(), TaskMergeStoreError> {
    transaction
        .execute(
            "INSERT INTO task_merge_outbound_progress(
                peer_device_id, policy_revision, policy_digest, policy_active
             ) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                peer_device_id,
                identity.revision,
                identity.digest,
                identity.active
            ],
        )
        .map_err(storage_error)?;
    Ok(())
}

pub fn update_outbound_policy_identity(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
) -> Result<(), TaskMergeStoreError> {
    transaction
        .execute(
            "UPDATE task_merge_outbound_progress
             SET policy_revision = ?2, policy_digest = ?3, policy_active = ?4,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE peer_device_id = ?1",
            rusqlite::params![
                peer_device_id,
                identity.revision,
                identity.digest,
                identity.active
            ],
        )
        .map_err(storage_error)?;
    Ok(())
}

pub fn claim_outbound_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    batch_id: &str,
    after: i64,
    through: i64,
) -> Result<bool, TaskMergeStoreError> {
    let changed = transaction
        .execute(
            "UPDATE task_merge_outbound_progress
             SET in_flight_batch_id = ?2, in_flight_after = ?3, in_flight_through = ?4,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE peer_device_id = ?1 AND acknowledged_through = ?3
               AND in_flight_batch_id IS NULL",
            rusqlite::params![peer_device_id, batch_id, after, through],
        )
        .map_err(storage_error)?;
    Ok(changed == 1)
}

pub fn acknowledge_outbound_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    batch_id: &str,
    through: i64,
    acknowledged_through: i64,
) -> Result<bool, TaskMergeStoreError> {
    let changed = transaction
        .execute(
            "UPDATE task_merge_outbound_progress
             SET acknowledged_through = ?2,
                 last_acknowledged_batch_id = ?3,
                 last_acknowledged_through = ?2,
                 in_flight_batch_id = NULL, in_flight_after = NULL, in_flight_through = NULL,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE peer_device_id = ?1 AND acknowledged_through = ?4
               AND in_flight_batch_id = ?3 AND in_flight_through = ?2",
            rusqlite::params![peer_device_id, through, batch_id, acknowledged_through],
        )
        .map_err(storage_error)?;
    Ok(changed == 1)
}

pub fn task_merge_watermark(connection: &Connection) -> Result<i64, TaskMergeStoreError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(position), 0) FROM task_merge_journal",
            [],
            |row| row.get(0),
        )
        .map_err(storage_error)
}
