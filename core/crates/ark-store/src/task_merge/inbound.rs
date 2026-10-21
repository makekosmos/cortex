use super::{storage_error, TaskMergeStoreError};
use ark_domain::sync_policy::PolicyIdentity;
use ark_domain::task_merge::TaskMergeOperation;
use ark_domain::task_merge_sync::{TaskMergeBatch, TaskMergeBatchAck, TaskMergeJournalRecord};
use rusqlite::{Connection, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};

const MAX_BATCH_ENTRIES: usize = 100;
const MAX_BATCH_BYTES: usize = 1_048_576 - 4_096;

#[derive(Debug)]
pub enum TaskMergeInboundError {
    Permission(String),
    Invalid(String),
    Storage(String),
    OperationIdCollision,
}

impl std::fmt::Display for TaskMergeInboundError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Permission(error) | Self::Invalid(error) | Self::Storage(error) => {
                formatter.write_str(error)
            }
            Self::OperationIdCollision => formatter.write_str("task_merge_operation_id_collision"),
        }
    }
}

impl std::error::Error for TaskMergeInboundError {}

impl From<TaskMergeStoreError> for TaskMergeInboundError {
    fn from(error: TaskMergeStoreError) -> Self {
        match error {
            TaskMergeStoreError::Invalid(error) => Self::Invalid(error),
            TaskMergeStoreError::Storage(error) => Self::Storage(error),
            TaskMergeStoreError::OperationIdCollision => Self::OperationIdCollision,
        }
    }
}

pub(crate) fn apply_inbound_task_merge_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    local_device_id: &str,
    identity: &PolicyIdentity,
    batch: &TaskMergeBatch,
) -> Result<TaskMergeBatchAck, TaskMergeInboundError> {
    let profile = crate::data_platform::load_sync_profile_for_sync(transaction, local_device_id)
        .map_err(TaskMergeInboundError::Storage)?;
    let actual_identity = ark_application::sync_policy::sync_profile_identity(profile.as_ref());
    if actual_identity != *identity {
        return Err(TaskMergeInboundError::Permission(
            "stale_or_unnegotiated_sync_policy".into(),
        ));
    }
    if let Some(profile) = profile.filter(|profile| profile.active) {
        let selected = profile.selective_profile();
        if ark_application::sync_policy::validate_task_merge_sync_policy(&selected).is_err() {
            return Err(TaskMergeInboundError::Permission(
                "task_merge_policy_requires_full_type_sync".into(),
            ));
        }
    }
    if !crate::device_registry::is_device_admitted(transaction, peer_device_id)
        .map_err(TaskMergeInboundError::Storage)?
        .unwrap_or(false)
    {
        return Err(TaskMergeInboundError::Permission(
            "sync_peer_not_admitted".into(),
        ));
    }
    let removed: Vec<String> = crate::sync_kv::get_sync_kv(transaction, "sync.removed_peers")
        .map_err(TaskMergeInboundError::Storage)?
        .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
        .transpose()
        .map_err(TaskMergeInboundError::Storage)?
        .unwrap_or_default();
    if removed
        .iter()
        .any(|removed_id| removed_id == peer_device_id)
    {
        return Err(TaskMergeInboundError::Permission(
            "sync_peer_removed".into(),
        ));
    }
    apply_inbound_batch(transaction, peer_device_id, identity, batch).map_err(Into::into)
}

fn apply_inbound_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    batch: &TaskMergeBatch,
) -> Result<TaskMergeBatchAck, TaskMergeStoreError> {
    validate_batch(identity, batch)?;
    let payload_digest = payload_digest(batch)?;
    let progress = load_inbound_progress(transaction, peer_device_id)?;

    if let Some(progress) = progress {
        if let Some(last_batch) = progress
            .last_batch
            .filter(|last_batch| last_batch.batch_id == batch.batch_id)
        {
            if last_batch.after != batch.after_position
                || last_batch.through != batch.through_position
                || last_batch.payload_digest != payload_digest
            {
                return Err(TaskMergeStoreError::Invalid(
                    "task_merge_inbound_batch_replay_mismatch".into(),
                ));
            }
            update_inbound_policy_identity(transaction, peer_device_id, identity)?;
            return Ok(TaskMergeBatchAck {
                batch_id: batch.batch_id.clone(),
                through_position: progress.received_through,
                replayed: true,
            });
        }
        if batch.after_position != progress.received_through {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_inbound_cursor_mismatch".into(),
            ));
        }
    } else if batch.after_position != 0 {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_inbound_cursor_mismatch".into(),
        ));
    }

    for entry in &batch.entries {
        match &entry.record {
            TaskMergeJournalRecord::Operation {
                operation,
                expected_conflict_digest,
            } => {
                append_remote_operation(
                    transaction,
                    operation,
                    expected_conflict_digest.as_deref(),
                )?;
            }
            TaskMergeJournalRecord::Bootstrap(bootstrap) => {
                super::apply_inbound_bootstrap(transaction, bootstrap)?;
            }
        }
    }

    record_inbound_batch(
        transaction,
        peer_device_id,
        identity,
        &TaskMergeInboundBatchReceipt {
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

fn append_remote_operation(
    transaction: &Transaction<'_>,
    operation: &TaskMergeOperation,
    expected_conflict_digest: Option<&str>,
) -> Result<(), TaskMergeStoreError> {
    let prepared = match super::check_operation(
        transaction,
        operation,
        expected_conflict_digest,
        super::TaskMergeOperationOrigin::Remote,
    )? {
        super::TaskMergeOperationCheck::Replay(_) => return Ok(()),
        super::TaskMergeOperationCheck::New(prepared) => prepared,
    };
    let object_id = prepared.operation().object_id.clone();
    super::persist_operation(transaction, prepared)?;
    super::materialize_task_if_ready(transaction, &object_id)?;
    Ok(())
}

fn validate_batch(
    identity: &PolicyIdentity,
    batch: &TaskMergeBatch,
) -> Result<(), TaskMergeStoreError> {
    if batch.identity != *identity
        || batch.batch_id.len() != 32
        || !batch.batch_id.bytes().all(|byte| byte.is_ascii_hexdigit())
        || batch.after_position < 0
        || batch.through_position <= batch.after_position
        || batch.entries.is_empty()
        || batch.entries.len() > MAX_BATCH_ENTRIES
    {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_inbound_batch_invalid".into(),
        ));
    }
    let count = usize::try_from(batch.through_position - batch.after_position)
        .map_err(|_| TaskMergeStoreError::Invalid("task_merge_inbound_batch_invalid".into()))?;
    if count != batch.entries.len()
        || batch.entries.iter().enumerate().any(|(offset, entry)| {
            i64::try_from(offset)
                .ok()
                .and_then(|offset| batch.after_position.checked_add(offset + 1))
                != Some(entry.position)
        })
    {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_inbound_positions_invalid".into(),
        ));
    }
    let encoded = serde_json::to_vec(batch)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    if encoded.len() > MAX_BATCH_BYTES {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_inbound_batch_too_large".into(),
        ));
    }
    Ok(())
}

fn payload_digest(batch: &TaskMergeBatch) -> Result<String, TaskMergeStoreError> {
    let payload =
        serde_json::to_vec(&(batch.after_position, batch.through_position, &batch.entries))
            .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(payload)))
}

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
            ));
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
