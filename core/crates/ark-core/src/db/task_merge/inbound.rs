use super::{append_remote_operation, TaskMergeError};
use crate::application::sync_storage::{PolicyIdentity, TaskMergeBatch, TaskMergeBatchAck};
use rusqlite::{OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

const MAX_BATCH_ENTRIES: usize = 100;
const MAX_BATCH_BYTES: usize = 1_048_576 - 4_096;

pub(crate) fn apply_inbound_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    batch: &TaskMergeBatch,
) -> Result<TaskMergeBatchAck, TaskMergeError> {
    validate_batch(identity, batch)?;
    let payload_digest = payload_digest(batch)?;
    let progress = load_progress(transaction, peer_device_id)?;

    if let Some(progress) = progress {
        if progress.last_batch_id.as_deref() == Some(&batch.batch_id) {
            if progress.last_batch_after != Some(batch.after_position)
                || progress.last_batch_through != Some(batch.through_position)
                || progress.last_batch_digest.as_deref() != Some(payload_digest.as_str())
            {
                return Err(TaskMergeError::Invalid(
                    "task_merge_inbound_batch_replay_mismatch".into(),
                ));
            }
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
            return Ok(TaskMergeBatchAck {
                batch_id: batch.batch_id.clone(),
                through_position: progress.received_through,
                replayed: true,
            });
        }
        if batch.after_position != progress.received_through {
            return Err(TaskMergeError::Invalid(
                "task_merge_inbound_cursor_mismatch".into(),
            ));
        }
    } else if batch.after_position != 0 {
        return Err(TaskMergeError::Invalid(
            "task_merge_inbound_cursor_mismatch".into(),
        ));
    }

    for entry in &batch.entries {
        append_remote_operation(
            transaction,
            entry.operation.clone(),
            entry.expected_conflict_digest.as_deref(),
        )?;
    }

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
                batch.through_position,
                batch.batch_id,
                batch.after_position,
                batch.through_position,
                payload_digest,
            ],
        )
        .map_err(storage_error)?;
    Ok(TaskMergeBatchAck {
        batch_id: batch.batch_id.clone(),
        through_position: batch.through_position,
        replayed: false,
    })
}

fn validate_batch(identity: &PolicyIdentity, batch: &TaskMergeBatch) -> Result<(), TaskMergeError> {
    if batch.identity != *identity
        || batch.batch_id.len() != 32
        || !batch.batch_id.bytes().all(|byte| byte.is_ascii_hexdigit())
        || batch.after_position < 0
        || batch.through_position <= batch.after_position
        || batch.entries.is_empty()
        || batch.entries.len() > MAX_BATCH_ENTRIES
    {
        return Err(TaskMergeError::Invalid(
            "task_merge_inbound_batch_invalid".into(),
        ));
    }
    let count = usize::try_from(batch.through_position - batch.after_position)
        .map_err(|_| TaskMergeError::Invalid("task_merge_inbound_batch_invalid".into()))?;
    if count != batch.entries.len()
        || batch.entries.iter().enumerate().any(|(offset, entry)| {
            i64::try_from(offset)
                .ok()
                .and_then(|offset| batch.after_position.checked_add(offset + 1))
                != Some(entry.position)
        })
    {
        return Err(TaskMergeError::Invalid(
            "task_merge_inbound_positions_invalid".into(),
        ));
    }
    let encoded =
        serde_json::to_vec(batch).map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    if encoded.len() > MAX_BATCH_BYTES {
        return Err(TaskMergeError::Invalid(
            "task_merge_inbound_batch_too_large".into(),
        ));
    }
    Ok(())
}

fn payload_digest(batch: &TaskMergeBatch) -> Result<String, TaskMergeError> {
    let payload =
        serde_json::to_vec(&(batch.after_position, batch.through_position, &batch.entries))
            .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(payload)))
}

#[derive(Debug)]
struct InboundProgress {
    received_through: i64,
    last_batch_id: Option<String>,
    last_batch_after: Option<i64>,
    last_batch_through: Option<i64>,
    last_batch_digest: Option<String>,
}

fn load_progress(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
) -> Result<Option<InboundProgress>, TaskMergeError> {
    transaction
        .query_row(
            "SELECT received_through, last_batch_id, last_batch_after,
                    last_batch_through, last_batch_digest
             FROM task_merge_inbound_progress WHERE peer_device_id = ?1",
            [peer_device_id],
            |row| {
                Ok(InboundProgress {
                    received_through: row.get(0)?,
                    last_batch_id: row.get(1)?,
                    last_batch_after: row.get(2)?,
                    last_batch_through: row.get(3)?,
                    last_batch_digest: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(storage_error)
}

fn storage_error(error: rusqlite::Error) -> TaskMergeError {
    TaskMergeError::Storage(error.to_string())
}
