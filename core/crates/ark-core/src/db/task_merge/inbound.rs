use super::{append_remote_operation, TaskMergeError};
use ark_application::sync_storage::{PolicyIdentity, TaskMergeBatch, TaskMergeBatchAck};
use rusqlite::Transaction;
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
    let progress = ark_store::task_merge::load_inbound_progress(transaction, peer_device_id)?;

    if let Some(progress) = progress {
        if let Some(last_batch) = progress
            .last_batch
            .filter(|last_batch| last_batch.batch_id == batch.batch_id)
        {
            if last_batch.after != batch.after_position
                || last_batch.through != batch.through_position
                || last_batch.payload_digest != payload_digest
            {
                return Err(TaskMergeError::Invalid(
                    "task_merge_inbound_batch_replay_mismatch".into(),
                ));
            }
            ark_store::task_merge::update_inbound_policy_identity(
                transaction,
                peer_device_id,
                identity,
            )?;
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
        match &entry.record {
            ark_application::sync_storage::TaskMergeJournalRecord::Operation {
                operation,
                expected_conflict_digest,
            } => {
                append_remote_operation(
                    transaction,
                    operation.clone(),
                    expected_conflict_digest.as_deref(),
                )?;
            }
            ark_application::sync_storage::TaskMergeJournalRecord::Bootstrap(bootstrap) => {
                super::append_bootstrap(transaction, bootstrap.clone())?;
            }
        }
    }

    ark_store::task_merge::record_inbound_batch(
        transaction,
        peer_device_id,
        identity,
        &ark_store::task_merge::TaskMergeInboundBatchReceipt {
            batch_id: batch.batch_id.clone(),
            after: batch.after_position,
            through: batch.through_position,
            payload_digest,
        },
    )?;
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
