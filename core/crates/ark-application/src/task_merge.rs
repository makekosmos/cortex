use crate::sync_policy::{
    default_sync_policy_identity, validate_task_merge_sync_policy, AcceptedPeerSyncPolicy,
    TaskMergeSyncPolicyError,
};
use ark_domain::PolicyIdentity;

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMergeConflictRead {
    pub conflict_set: Option<TaskConflictSet>,
    pub dependencies_pending: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMergeAppendResult {
    pub conflict_set: Option<TaskConflictSet>,
    pub dependencies_pending: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskMergeJournalPage {
    pub through_position: i64,
    pub entries: Vec<TaskMergeJournalEntry>,
}
use ark_domain::task_merge::TaskConflictSet;
use ark_domain::task_merge_sync::{TaskMergeBatch, TaskMergeBatchAck, TaskMergeJournalEntry};

const MAX_TASK_MERGE_BATCH_BYTES: usize = 1_048_576 - 4_096;

#[derive(Debug, PartialEq, Eq)]
pub enum TaskMergeDeliveryPolicyError {
    StaleOrUnnegotiated,
    FullTypeRequired,
    FilterUnsupported,
}

pub fn validate_task_merge_delivery_policy(
    current: Option<&AcceptedPeerSyncPolicy>,
    expected: &PolicyIdentity,
) -> Result<(), TaskMergeDeliveryPolicyError> {
    let current_identity =
        current.map_or_else(default_sync_policy_identity, |policy| policy.identity());
    if current_identity != *expected {
        return Err(TaskMergeDeliveryPolicyError::StaleOrUnnegotiated);
    }
    if let Some(policy) = current.filter(|policy| policy.active) {
        match validate_task_merge_sync_policy(&policy.selective_profile()) {
            Ok(()) => {}
            Err(TaskMergeSyncPolicyError::FullTypeRequired) => {
                return Err(TaskMergeDeliveryPolicyError::FullTypeRequired);
            }
            Err(TaskMergeSyncPolicyError::FilterUnsupported) => {
                return Err(TaskMergeDeliveryPolicyError::FilterUnsupported);
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum TaskMergeBatchBuildError {
    Serialization(String),
    OperationTooLarge,
}

impl std::fmt::Display for TaskMergeBatchBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialization(error) => formatter.write_str(error),
            Self::OperationTooLarge => {
                formatter.write_str("task_merge_operation_exceeds_batch_limit")
            }
        }
    }
}

pub fn build_task_merge_batch(
    batch_id: String,
    identity: &PolicyIdentity,
    after_position: i64,
    entries: Vec<TaskMergeJournalEntry>,
) -> Result<TaskMergeBatch, TaskMergeBatchBuildError> {
    let empty_batch = TaskMergeBatch {
        batch_id: batch_id.clone(),
        identity: identity.clone(),
        after_position,
        through_position: after_position,
        entries: Vec::new(),
    };
    let empty_encoded = serde_json::to_vec(&empty_batch)
        .map_err(|error| TaskMergeBatchBuildError::Serialization(error.to_string()))?;
    let fixed_bytes = empty_encoded.len().saturating_sub(2);
    let mut entries_bytes = 2;
    let mut fitting_entries = 0;
    for entry in &entries {
        let entry_bytes = serde_json::to_vec(entry)
            .map_err(|error| TaskMergeBatchBuildError::Serialization(error.to_string()))?
            .len();
        let next_bytes = entries_bytes + usize::from(fitting_entries > 0) + entry_bytes;
        if fixed_bytes + next_bytes > MAX_TASK_MERGE_BATCH_BYTES {
            break;
        }
        entries_bytes = next_bytes;
        fitting_entries += 1;
    }
    if fitting_entries == 0 {
        return Err(TaskMergeBatchBuildError::OperationTooLarge);
    }
    let entries = entries
        .into_iter()
        .take(fitting_entries)
        .collect::<Vec<_>>();
    let through_position = entries
        .last()
        .ok_or(TaskMergeBatchBuildError::OperationTooLarge)?
        .position;
    Ok(TaskMergeBatch {
        batch_id,
        identity: identity.clone(),
        after_position,
        through_position,
        entries,
    })
}

#[derive(Debug)]
pub enum TaskMergePrepareError {
    PolicyRejected,
    Failure(String),
}

#[async_trait::async_trait]
pub trait TaskMergeDeliveryPort: Send + Sync {
    async fn prepare_task_merge_batch(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        limit: usize,
    ) -> Result<Option<TaskMergeBatch>, TaskMergePrepareError>;

    async fn acknowledge_task_merge_batch(
        &self,
        peer_device_id: &str,
        expected_identity: &PolicyIdentity,
        batch_id: &str,
        through_position: i64,
    ) -> Result<i64, String>;

    async fn apply_task_merge_batch(
        &self,
        peer_device_id: &str,
        local_device_id: &str,
        expected_identity: &PolicyIdentity,
        batch: &TaskMergeBatch,
    ) -> Result<TaskMergeBatchAck, String>;
}
