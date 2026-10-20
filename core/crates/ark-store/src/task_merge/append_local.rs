use super::{
    check_operation, materialize_task_if_ready, persist_operation, read_conflict_set,
    task_causal_dependencies_pending, TaskMergeOperationCheck, TaskMergeOperationOrigin,
    TaskMergeStoreError,
};
use ark_application::task_merge::TaskMergeAppendResult;
use ark_domain::task_merge::TaskMergeOperation;
use rusqlite::Connection;

#[derive(Debug)]
pub enum TaskMergeAppendError {
    OperationIdCollision,
    ConflictSetChanged,
    ConflictDigestRequired,
    Invalid(String),
    Storage(String),
}

impl std::fmt::Display for TaskMergeAppendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OperationIdCollision => formatter.write_str("task_merge_operation_id_collision"),
            Self::ConflictSetChanged => formatter.write_str("task_merge_conflict_set_changed"),
            Self::ConflictDigestRequired => {
                formatter.write_str("task_merge_conflict_digest_required")
            }
            Self::Invalid(error) | Self::Storage(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for TaskMergeAppendError {}

impl From<TaskMergeStoreError> for TaskMergeAppendError {
    fn from(error: TaskMergeStoreError) -> Self {
        match error {
            TaskMergeStoreError::Invalid(error) => Self::Invalid(error),
            TaskMergeStoreError::Storage(error) => Self::Storage(error),
            TaskMergeStoreError::OperationIdCollision => Self::OperationIdCollision,
        }
    }
}

pub fn append_local_operation(
    transaction: &Connection,
    operation: TaskMergeOperation,
    expected_conflict_digest: Option<&str>,
) -> Result<TaskMergeAppendResult, TaskMergeAppendError> {
    let prepared = match check_operation(
        transaction,
        &operation,
        expected_conflict_digest,
        TaskMergeOperationOrigin::Local,
    )? {
        TaskMergeOperationCheck::Replay(read) => {
            return Ok(TaskMergeAppendResult {
                conflict_set: read.conflict_set,
                dependencies_pending: read.dependencies_pending,
                replayed: true,
            });
        }
        TaskMergeOperationCheck::New(prepared) => prepared,
    };
    let operation = prepared.operation().clone();
    let current = read_conflict_set(transaction, &operation.object_id, operation.unit)?;
    let resolves_conflict = current.conflict_set.as_ref().is_some_and(|set| {
        set.has_conflict()
            && set
                .variants
                .iter()
                .all(|variant| operation.parents.contains(&variant.id))
    });
    if resolves_conflict && expected_conflict_digest.is_none() {
        return Err(TaskMergeAppendError::ConflictDigestRequired);
    }
    if let Some(expected) = expected_conflict_digest {
        if current.dependencies_pending
            || current.conflict_set.as_ref().map(|set| set.digest.as_str()) != Some(expected)
        {
            return Err(TaskMergeAppendError::ConflictSetChanged);
        }
    }

    persist_operation(transaction, prepared)?;
    materialize_task_if_ready(transaction, &operation.object_id)?;

    let current = read_conflict_set(transaction, &operation.object_id, operation.unit)?;
    Ok(TaskMergeAppendResult {
        conflict_set: current.conflict_set,
        dependencies_pending: current.dependencies_pending
            || task_causal_dependencies_pending(transaction, &operation.object_id)?,
        replayed: false,
    })
}
