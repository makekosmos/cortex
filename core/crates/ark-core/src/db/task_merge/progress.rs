use super::{require_delivery_policy, TaskMergeError};
use ark_application::sync_storage::{PolicyIdentity, TaskMergeBatch};
use rusqlite::Transaction;

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

    let progress = ark_store::task_merge::load_outbound_progress(transaction, peer_device_id)?;
    let acknowledged_through = if let Some(progress) = progress {
        if progress.identity != *identity {
            ark_store::task_merge::update_outbound_policy_identity(
                transaction,
                peer_device_id,
                identity,
            )?;
            if let Some(in_flight) = progress.in_flight {
                return load_in_flight_batch(
                    transaction,
                    in_flight.batch_id,
                    identity,
                    in_flight.after,
                    in_flight.through,
                )
                .map(Some);
            }
            progress.acknowledged_through
        } else if let Some(in_flight) = progress.in_flight {
            return load_in_flight_batch(
                transaction,
                in_flight.batch_id,
                identity,
                in_flight.after,
                in_flight.through,
            )
            .map(Some);
        } else {
            progress.acknowledged_through
        }
    } else {
        ark_store::task_merge::create_outbound_progress(transaction, peer_device_id, identity)?;
        0
    };

    let watermark = ark_store::task_merge::task_merge_watermark(transaction)?;
    if acknowledged_through > watermark {
        return Err(TaskMergeError::Storage(
            "task_merge_progress_ahead_of_journal".into(),
        ));
    }
    if acknowledged_through == watermark {
        return Ok(None);
    }
    let page = ark_store::task_merge::read_journal_page(
        transaction,
        acknowledged_through,
        watermark,
        limit,
    )?;
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
    if !ark_store::task_merge::claim_outbound_batch(
        transaction,
        peer_device_id,
        &batch_id,
        acknowledged_through,
        through_position,
    )? {
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
    after: i64,
    through: i64,
) -> Result<TaskMergeBatch, TaskMergeError> {
    let page = ark_store::task_merge::read_journal_page(
        transaction,
        after,
        through,
        MAX_TASK_MERGE_PAGE_SIZE,
    )?;
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
    let progress = ark_store::task_merge::load_outbound_progress(transaction, peer_device_id)?
        .ok_or_else(|| TaskMergeError::Invalid("task_merge_batch_not_pending".into()))?;
    if progress.identity != *identity {
        return Err(TaskMergeError::Permission(
            "stale_or_unnegotiated_sync_policy".into(),
        ));
    }
    if progress
        .last_acknowledged
        .as_ref()
        .is_some_and(|ack| ack.batch_id == batch_id && ack.through == through_position)
    {
        return Ok(progress.acknowledged_through);
    }
    if !progress.in_flight.as_ref().is_some_and(|in_flight| {
        in_flight.batch_id == batch_id
            && in_flight.through == through_position
            && in_flight.after == progress.acknowledged_through
    }) {
        return Err(TaskMergeError::Invalid(
            "task_merge_batch_ack_mismatch".into(),
        ));
    }
    if !ark_store::task_merge::acknowledge_outbound_batch(
        transaction,
        peer_device_id,
        batch_id,
        through_position,
        progress.acknowledged_through,
    )? {
        return Err(TaskMergeError::Invalid(
            "task_merge_batch_ack_mismatch".into(),
        ));
    }
    Ok(through_position)
}
