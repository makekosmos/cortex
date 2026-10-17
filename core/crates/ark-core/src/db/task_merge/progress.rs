use super::{read_journal_page, require_delivery_policy, TaskMergeError};
use ark_application::sync_storage::{PolicyIdentity, TaskMergeBatch};
use rusqlite::{OptionalExtension, Transaction};

const MAX_TASK_MERGE_PAGE_SIZE: usize = 100;
// Leave room for the protocol discriminator and transport envelope.
const MAX_TASK_MERGE_BATCH_BYTES: usize = 1_048_576 - 4_096;

pub(crate) fn prepare_outbound_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    limit: usize,
) -> Result<Option<TaskMergeBatch>, TaskMergeError> {
    require_delivery_policy(transaction, peer_device_id, identity)?;
    if limit == 0 || limit > MAX_TASK_MERGE_PAGE_SIZE {
        return Err(TaskMergeError::Invalid(
            "task_merge_page_limit_exceeded".into(),
        ));
    }

    let progress = load_progress(transaction, peer_device_id)?;
    let acknowledged_through = if let Some(progress) = progress {
        if progress.revision != identity.revision
            || progress.digest != identity.digest
            || progress.active != identity.active
        {
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
            if let Some(batch_id) = progress.in_flight_batch_id {
                return load_in_flight_batch(
                    transaction,
                    batch_id,
                    identity,
                    progress.in_flight_after,
                    progress.in_flight_through,
                )
                .map(Some);
            }
            progress.acknowledged_through
        } else if let Some(batch_id) = progress.in_flight_batch_id {
            return load_in_flight_batch(
                transaction,
                batch_id,
                identity,
                progress.in_flight_after,
                progress.in_flight_through,
            )
            .map(Some);
        } else {
            progress.acknowledged_through
        }
    } else {
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
        0
    };

    let watermark = task_merge_watermark(transaction)?;
    if acknowledged_through > watermark {
        return Err(TaskMergeError::Storage(
            "task_merge_progress_ahead_of_journal".into(),
        ));
    }
    if acknowledged_through == watermark {
        return Ok(None);
    }
    let page = read_journal_page(transaction, acknowledged_through, watermark, limit)?;
    if page.entries.is_empty() {
        return Ok(None);
    }
    let batch_id = format!("{:032x}", rand::random::<u128>());
    let batch = fit_batch(
        batch_id.clone(),
        identity,
        acknowledged_through,
        page.entries,
    )?;
    let through_position = batch.through_position;
    let changed = transaction
        .execute(
            "UPDATE task_merge_outbound_progress
             SET in_flight_batch_id = ?2, in_flight_after = ?3, in_flight_through = ?4,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE peer_device_id = ?1 AND acknowledged_through = ?3
               AND in_flight_batch_id IS NULL",
            rusqlite::params![
                peer_device_id,
                batch_id,
                acknowledged_through,
                through_position
            ],
        )
        .map_err(storage_error)?;
    if changed != 1 {
        return Err(TaskMergeError::Storage(
            "task_merge_progress_changed_during_prepare".into(),
        ));
    }
    Ok(Some(batch))
}

fn load_in_flight_batch(
    transaction: &Transaction<'_>,
    batch_id: String,
    identity: &PolicyIdentity,
    after: Option<i64>,
    through: Option<i64>,
) -> Result<TaskMergeBatch, TaskMergeError> {
    let after =
        after.ok_or_else(|| TaskMergeError::Storage("task_merge_progress_corrupt".into()))?;
    let through =
        through.ok_or_else(|| TaskMergeError::Storage("task_merge_progress_corrupt".into()))?;
    let page = read_journal_page(transaction, after, through, MAX_TASK_MERGE_PAGE_SIZE)?;
    if page.entries.last().map(|entry| entry.position) != Some(through) {
        return Err(TaskMergeError::Storage(
            "task_merge_in_flight_batch_missing_entries".into(),
        ));
    }
    let batch = fit_batch(batch_id, identity, after, page.entries)?;
    if batch.through_position != through {
        return Err(TaskMergeError::Storage(
            "task_merge_in_flight_batch_size_changed".into(),
        ));
    }
    Ok(batch)
}

fn fit_batch(
    batch_id: String,
    identity: &PolicyIdentity,
    after_position: i64,
    entries: Vec<ark_application::sync_storage::TaskMergeJournalEntry>,
) -> Result<TaskMergeBatch, TaskMergeError> {
    let empty_batch = TaskMergeBatch {
        batch_id: batch_id.clone(),
        identity: identity.clone(),
        after_position,
        through_position: after_position,
        entries: Vec::new(),
    };
    let empty_encoded = serde_json::to_vec(&empty_batch)
        .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    let fixed_bytes = empty_encoded.len().saturating_sub(2);
    let mut entries_bytes = 2;
    let mut fitting_entries = 0;
    for entry in &entries {
        let entry_bytes = serde_json::to_vec(entry)
            .map_err(|error| TaskMergeError::Invalid(error.to_string()))?
            .len();
        let next_bytes = entries_bytes + usize::from(fitting_entries > 0) + entry_bytes;
        if fixed_bytes + next_bytes > MAX_TASK_MERGE_BATCH_BYTES {
            break;
        }
        entries_bytes = next_bytes;
        fitting_entries += 1;
    }
    if fitting_entries == 0 {
        return Err(TaskMergeError::Invalid(
            "task_merge_operation_exceeds_batch_limit".into(),
        ));
    }
    let entries = entries
        .into_iter()
        .take(fitting_entries)
        .collect::<Vec<_>>();
    let through_position = entries.last().map(|entry| entry.position).ok_or_else(|| {
        TaskMergeError::Invalid("task_merge_operation_exceeds_batch_limit".into())
    })?;
    Ok(TaskMergeBatch {
        batch_id,
        identity: identity.clone(),
        after_position,
        through_position,
        entries,
    })
}

pub(crate) fn acknowledge_outbound_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    batch_id: &str,
    through_position: i64,
) -> Result<i64, TaskMergeError> {
    require_delivery_policy(transaction, peer_device_id, identity)?;
    let progress = load_progress(transaction, peer_device_id)?
        .ok_or_else(|| TaskMergeError::Invalid("task_merge_batch_not_pending".into()))?;
    if progress.revision != identity.revision
        || progress.digest != identity.digest
        || progress.active != identity.active
    {
        return Err(TaskMergeError::Permission(
            "stale_or_unnegotiated_sync_policy".into(),
        ));
    }
    if progress.last_acknowledged_batch_id.as_deref() == Some(batch_id)
        && progress.last_acknowledged_through == Some(through_position)
    {
        return Ok(progress.acknowledged_through);
    }
    if progress.in_flight_batch_id.as_deref() != Some(batch_id)
        || progress.in_flight_through != Some(through_position)
        || progress.in_flight_after != Some(progress.acknowledged_through)
    {
        return Err(TaskMergeError::Invalid(
            "task_merge_batch_ack_mismatch".into(),
        ));
    }
    transaction
        .execute(
            "UPDATE task_merge_outbound_progress
             SET acknowledged_through = ?2,
                 last_acknowledged_batch_id = ?3,
                 last_acknowledged_through = ?2,
                 in_flight_batch_id = NULL, in_flight_after = NULL, in_flight_through = NULL,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE peer_device_id = ?1 AND acknowledged_through = ?4
               AND in_flight_batch_id = ?3 AND in_flight_through = ?2",
            rusqlite::params![
                peer_device_id,
                through_position,
                batch_id,
                progress.acknowledged_through
            ],
        )
        .map_err(storage_error)?;
    Ok(through_position)
}

struct StoredProgress {
    revision: i64,
    digest: String,
    active: bool,
    acknowledged_through: i64,
    in_flight_batch_id: Option<String>,
    in_flight_after: Option<i64>,
    in_flight_through: Option<i64>,
    last_acknowledged_batch_id: Option<String>,
    last_acknowledged_through: Option<i64>,
}

fn load_progress(
    connection: &rusqlite::Connection,
    peer_device_id: &str,
) -> Result<Option<StoredProgress>, TaskMergeError> {
    connection
        .query_row(
            "SELECT policy_revision, policy_digest, policy_active, acknowledged_through,
                    in_flight_batch_id, in_flight_after, in_flight_through,
                    last_acknowledged_batch_id, last_acknowledged_through
             FROM task_merge_outbound_progress WHERE peer_device_id = ?1",
            [peer_device_id],
            |row| {
                Ok(StoredProgress {
                    revision: row.get(0)?,
                    digest: row.get(1)?,
                    active: row.get(2)?,
                    acknowledged_through: row.get(3)?,
                    in_flight_batch_id: row.get(4)?,
                    in_flight_after: row.get(5)?,
                    in_flight_through: row.get(6)?,
                    last_acknowledged_batch_id: row.get(7)?,
                    last_acknowledged_through: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(storage_error)
}

fn task_merge_watermark(connection: &rusqlite::Connection) -> Result<i64, TaskMergeError> {
    connection
        .query_row(
            "SELECT COALESCE(MAX(position), 0) FROM task_merge_journal",
            [],
            |row| row.get(0),
        )
        .map_err(storage_error)
}

fn storage_error(error: rusqlite::Error) -> TaskMergeError {
    TaskMergeError::Storage(error.to_string())
}
