use super::{
    claim_outbound_batch, create_outbound_progress, load_outbound_progress, read_journal_page,
    task_merge_watermark, update_outbound_policy_identity, TaskMergeStoreError,
};
use ark_application::sync_policy::PolicyIdentity;
use ark_application::sync_storage::TaskMergeBatch;
use rusqlite::Transaction;

#[derive(Debug)]
pub enum TaskMergeOutboundError {
    Permission(String),
    Invalid(String),
    Storage(String),
}

impl std::fmt::Display for TaskMergeOutboundError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Permission(error) | Self::Invalid(error) | Self::Storage(error) => {
                formatter.write_str(error)
            }
        }
    }
}

impl std::error::Error for TaskMergeOutboundError {}

impl From<TaskMergeStoreError> for TaskMergeOutboundError {
    fn from(error: TaskMergeStoreError) -> Self {
        match error {
            TaskMergeStoreError::Invalid(error) => Self::Invalid(error),
            TaskMergeStoreError::Storage(error) => Self::Storage(error),
            TaskMergeStoreError::OperationIdCollision => {
                Self::Storage("task_merge_operation_id_collision".into())
            }
        }
    }
}

pub(crate) fn prepare_task_merge_outbound_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    limit: usize,
) -> Result<Option<TaskMergeBatch>, TaskMergeOutboundError> {
    require_delivery_policy(transaction, peer_device_id, identity)?;
    if limit == 0 || limit > ark_application::task_merge::MAX_TASK_MERGE_PAGE_SIZE {
        return Err(TaskMergeOutboundError::Invalid(
            "task_merge_page_limit_exceeded".into(),
        ));
    }

    let progress = load_outbound_progress(transaction, peer_device_id)?;
    let acknowledged_through = if let Some(progress) = progress {
        if progress.identity != *identity {
            update_outbound_policy_identity(transaction, peer_device_id, identity)?;
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
        create_outbound_progress(transaction, peer_device_id, identity)?;
        0
    };

    let watermark = task_merge_watermark(transaction)?;
    if acknowledged_through > watermark {
        return Err(TaskMergeOutboundError::Storage(
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
    let batch = ark_application::task_merge::build_task_merge_batch(
        batch_id.clone(),
        identity,
        acknowledged_through,
        page.entries,
    )
    .map_err(|error| TaskMergeOutboundError::Invalid(error.to_string()))?;
    if !claim_outbound_batch(
        transaction,
        peer_device_id,
        &batch_id,
        acknowledged_through,
        batch.through_position,
    )? {
        return Err(TaskMergeOutboundError::Storage(
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
) -> Result<TaskMergeBatch, TaskMergeOutboundError> {
    let page = read_journal_page(
        transaction,
        after,
        through,
        ark_application::task_merge::MAX_TASK_MERGE_PAGE_SIZE,
    )?;
    if page.entries.last().map(|entry| entry.position) != Some(through) {
        return Err(TaskMergeOutboundError::Storage(
            "task_merge_in_flight_batch_missing_entries".into(),
        ));
    }
    let batch = ark_application::task_merge::build_task_merge_batch(
        batch_id,
        identity,
        after,
        page.entries,
    )
    .map_err(|error| TaskMergeOutboundError::Invalid(error.to_string()))?;
    if batch.through_position != through {
        return Err(TaskMergeOutboundError::Storage(
            "task_merge_in_flight_batch_size_changed".into(),
        ));
    }
    Ok(batch)
}

pub(crate) fn acknowledge_task_merge_outbound_batch(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    identity: &PolicyIdentity,
    batch_id: &str,
    through_position: i64,
) -> Result<i64, TaskMergeOutboundError> {
    require_delivery_policy(transaction, peer_device_id, identity)?;
    let progress = load_outbound_progress(transaction, peer_device_id)?
        .ok_or_else(|| TaskMergeOutboundError::Invalid("task_merge_batch_not_pending".into()))?;
    if progress.identity != *identity {
        return Err(TaskMergeOutboundError::Permission(
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
        return Err(TaskMergeOutboundError::Invalid(
            "task_merge_batch_ack_mismatch".into(),
        ));
    }
    if !super::progress::acknowledge_outbound_batch(
        transaction,
        peer_device_id,
        batch_id,
        through_position,
        progress.acknowledged_through,
    )? {
        return Err(TaskMergeOutboundError::Invalid(
            "task_merge_batch_ack_mismatch".into(),
        ));
    }
    Ok(through_position)
}

fn require_delivery_policy(
    transaction: &Transaction<'_>,
    peer_device_id: &str,
    expected_identity: &PolicyIdentity,
) -> Result<(), TaskMergeOutboundError> {
    let policy = crate::data_platform::load_accepted_peer_sync_policy(transaction, peer_device_id)
        .map_err(TaskMergeOutboundError::Storage)?;
    ark_application::task_merge::validate_task_merge_delivery_policy(
        policy.as_ref(),
        expected_identity,
    )
    .map_err(|error| {
        let message = match error {
            ark_application::task_merge::TaskMergeDeliveryPolicyError::StaleOrUnnegotiated => {
                "stale_or_unnegotiated_sync_policy"
            }
            ark_application::task_merge::TaskMergeDeliveryPolicyError::FullTypeRequired => {
                "task_merge_policy_requires_full_type_sync"
            }
            ark_application::task_merge::TaskMergeDeliveryPolicyError::FilterUnsupported => {
                "task_merge_policy_filter_unsupported"
            }
        };
        TaskMergeOutboundError::Permission(message.into())
    })
}
