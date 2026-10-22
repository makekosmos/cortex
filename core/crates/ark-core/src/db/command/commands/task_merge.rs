use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};
use ark_domain::sync_policy::PolicyIdentity;
use ark_domain::task_merge::{TaskMergeOperation, TaskMergeUnit};
use ark_domain::task_merge_sync::TaskMergeBatch;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AppendTaskMergeOperationRequest {
    pub(crate) operation: TaskMergeOperation,
    pub(crate) expected_conflict_digest: Option<String>,
}

impl AppendTaskMergeOperationRequest {
    pub(crate) fn new(
        operation: TaskMergeOperation,
        expected_conflict_digest: Option<String>,
    ) -> Self {
        Self {
            operation,
            expected_conflict_digest,
        }
    }

    pub(crate) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PrepareTaskMergeBatchCommand {
    pub(crate) peer_id: String,
    pub(crate) expected_identity: PolicyIdentity,
    pub(crate) limit: usize,
}

impl PrepareTaskMergeBatchCommand {
    pub(crate) fn new(peer_id: String, expected_identity: PolicyIdentity, limit: usize) -> Self {
        Self {
            peer_id,
            expected_identity,
            limit,
        }
    }
}

impl PrepareTaskMergeBatchCommand {
    pub(crate) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AcknowledgeTaskMergeBatchCommand {
    pub(crate) peer_id: String,
    pub(crate) expected_identity: PolicyIdentity,
    pub(crate) batch_id: String,
    pub(crate) through_position: i64,
}

impl AcknowledgeTaskMergeBatchCommand {
    pub(crate) fn new(
        peer_id: String,
        expected_identity: PolicyIdentity,
        batch_id: String,
        through_position: i64,
    ) -> Self {
        Self {
            peer_id,
            expected_identity,
            batch_id,
            through_position,
        }
    }
}

impl AcknowledgeTaskMergeBatchCommand {
    pub(crate) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApplyTaskMergeBatchCommand {
    pub(crate) peer_id: String,
    pub(crate) local_device_id: String,
    pub(crate) expected_identity: PolicyIdentity,
    pub(crate) batch: TaskMergeBatch,
}

impl ApplyTaskMergeBatchCommand {
    pub(crate) fn new(
        peer_id: String,
        local_device_id: String,
        expected_identity: PolicyIdentity,
        batch: TaskMergeBatch,
    ) -> Self {
        Self {
            peer_id,
            local_device_id,
            expected_identity,
            batch,
        }
    }

    pub(crate) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TaskConflictSetQuery {
    object_id: String,
    unit: TaskMergeUnit,
}

impl TaskConflictSetQuery {
    pub(crate) fn new(object_id: String, unit: TaskMergeUnit) -> Self {
        Self { object_id, unit }
    }
}

impl sealed::Sealed for TaskConflictSetQuery {}

impl StoreReadQuery for TaskConflictSetQuery {
    type Output = ark_application::task_merge::TaskMergeConflictRead;

    fn class(&self) -> CommandClass {
        CommandClass::Interactive
    }

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        capabilities
            .task_merge()
            .conflict_set(&self.object_id, self.unit)
            .map_err(map_task_merge_error)
    }
}

fn map_task_merge_error(error: ark_store::task_merge::TaskMergeStoreError) -> DbError {
    match error {
        ark_store::task_merge::TaskMergeStoreError::OperationIdCollision => {
            DbError::Conflict(error.to_string())
        }
        ark_store::task_merge::TaskMergeStoreError::Invalid(error) => DbError::Invalid(error),
        ark_store::task_merge::TaskMergeStoreError::Storage(error) => DbError::Storage(error),
    }
}
