use super::{storage_error, TaskMergeStoreError};
use ark_application::sync_policy::PolicyIdentity;
use rusqlite::{Connection, OptionalExtension, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMergeInboundProgress {
    pub received_through: i64,
    pub last_batch: Option<TaskMergeInboundBatchReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskMergeInboundBatchReceipt {
    pub batch_id: String,
    pub after: i64,
    pub through: i64,
    pub payload_digest: String,
}

pub fn load_inbound_progress(
    connection: &Connection,
    peer_device_id: &str,
) -> Result<Option<TaskMergeInboundProgress>, TaskMergeStoreError> {
    let stored = connection
        .query_row(
            "SELECT received_through, last_batch_id, last_batch_after,
                    last_batch_through, last_batch_digest
             FROM task_merge_inbound_progress WHERE peer_device_id = ?1",
            [peer_device_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(storage_error)?;
    let Some((received_through, batch_id, after, through, payload_digest)) = stored else {
        return Ok(None);
    };
    let last_batch = match (batch_id, after, through, payload_digest) {
        (None, None, None, None) => None,
        (Some(batch_id), Some(after), Some(through), Some(payload_digest)) => {
            Some(TaskMergeInboundBatchReceipt {
                batch_id,
                after,
                through,
                payload_digest,
            })
        }
        _ => {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_inbound_progress_corrupt".into(),
            ))
        }
    };
    Ok(Some(TaskMergeInboundProgress {
        received_through,
        last_batch,
    }))
}

pub fn update_inbound_policy_identity(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
) -> Result<(), TaskMergeStoreError> {
    transaction
        .execute(
            "UPDATE task_merge_inbound_progress
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

pub fn record_inbound_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    receipt: &TaskMergeInboundBatchReceipt,
) -> Result<(), TaskMergeStoreError> {
    transaction
        .execute(
            "INSERT INTO task_merge_inbound_progress(
                peer_device_id, policy_revision, policy_digest, policy_active,
                received_through, last_batch_id, last_batch_after, last_batch_through,
                last_batch_digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(peer_device_id) DO UPDATE SET
                policy_revision = excluded.policy_revision,
                policy_digest = excluded.policy_digest,
                policy_active = excluded.policy_active,
                received_through = excluded.received_through,
                last_batch_id = excluded.last_batch_id,
                last_batch_after = excluded.last_batch_after,
                last_batch_through = excluded.last_batch_through,
                last_batch_digest = excluded.last_batch_digest,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            rusqlite::params![
                peer_device_id,
                identity.revision,
                identity.digest,
                identity.active,
                receipt.through,
                receipt.batch_id,
                receipt.after,
                receipt.through,
                receipt.payload_digest,
            ],
        )
        .map_err(storage_error)?;
    Ok(())
}
