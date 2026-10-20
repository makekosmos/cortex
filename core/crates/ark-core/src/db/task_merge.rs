use rusqlite::Connection;

#[path = "task_merge/progress.rs"]
mod progress;
pub(crate) use progress::{acknowledge_outbound_batch, prepare_outbound_batch};

#[derive(Debug, thiserror::Error)]
pub(crate) enum TaskMergeError {
    #[error("task_merge_operation_id_collision")]
    OperationIdCollision,
    #[error("{0}")]
    Permission(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Storage(String),
}

impl From<ark_store::task_merge::TaskMergeStoreError> for TaskMergeError {
    fn from(error: ark_store::task_merge::TaskMergeStoreError) -> Self {
        match error {
            ark_store::task_merge::TaskMergeStoreError::Invalid(error) => Self::Invalid(error),
            ark_store::task_merge::TaskMergeStoreError::Storage(error) => Self::Storage(error),
            ark_store::task_merge::TaskMergeStoreError::OperationIdCollision => {
                Self::OperationIdCollision
            }
        }
    }
}

pub(crate) fn require_delivery_policy(
    connection: &Connection,
    peer_device_id: &str,
    expected_identity: &ark_application::sync_storage::PolicyIdentity,
) -> Result<(), TaskMergeError> {
    let policy =
        ark_store::data_platform::load_accepted_peer_sync_policy(connection, peer_device_id)
            .map_err(TaskMergeError::Storage)?;
    let identity = policy.as_ref().map_or_else(
        ark_application::sync_policy::default_sync_policy_identity,
        |policy| policy.identity(),
    );
    if identity != *expected_identity {
        return Err(TaskMergeError::Permission(
            "stale_or_unnegotiated_sync_policy".into(),
        ));
    }
    if let Some(policy) = policy.filter(|policy| policy.active) {
        let profile = policy.selective_profile();
        match ark_application::sync_policy::validate_task_merge_sync_policy(&profile) {
            Ok(()) => {}
            Err(ark_application::sync_policy::TaskMergeSyncPolicyError::FullTypeRequired) => {
                return Err(TaskMergeError::Permission(
                    "task_merge_policy_requires_full_type_sync".into(),
                ));
            }
            Err(ark_application::sync_policy::TaskMergeSyncPolicyError::FilterUnsupported) => {
                return Err(TaskMergeError::Permission(
                    "task_merge_policy_filter_unsupported".into(),
                ));
            }
        }
    }
    Ok(())
}
