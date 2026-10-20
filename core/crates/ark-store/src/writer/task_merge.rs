#[cfg(debug_assertions)]
use super::{Connection, TransactionFailure};
use super::{SqliteWriter, StoreTransactionFailure};
use crate::task_merge::TaskMergeAppendError;
use ark_application::sync_policy::PolicyIdentity;
use ark_application::sync_storage::{TaskMergeBatch, TaskMergeBatchAck};
use ark_application::task_merge::TaskMergeAppendResult;
use ark_domain::task_merge::TaskMergeOperation;

impl SqliteWriter {
    pub fn apply_inbound_task_merge_batch(
        &mut self,
        peer_device_id: &str,
        local_device_id: &str,
        identity: PolicyIdentity,
        batch: TaskMergeBatch,
    ) -> Result<
        TaskMergeBatchAck,
        super::TransactionFailure<crate::task_merge::TaskMergeInboundError>,
    > {
        self.immediate(|transaction| {
            crate::task_merge::apply_inbound_task_merge_batch(
                transaction,
                peer_device_id,
                local_device_id,
                &identity,
                &batch,
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn apply_inbound_task_merge_batch_on_connection(
        connection: &mut Connection,
        peer_device_id: &str,
        local_device_id: &str,
        identity: PolicyIdentity,
        batch: TaskMergeBatch,
    ) -> Result<TaskMergeBatchAck, TransactionFailure<crate::task_merge::TaskMergeInboundError>>
    {
        Self::immediate_on_connection(connection, |transaction| {
            crate::task_merge::apply_inbound_task_merge_batch(
                transaction,
                peer_device_id,
                local_device_id,
                &identity,
                &batch,
            )
        })
    }

    pub fn append_local_task_merge_operation(
        &mut self,
        operation: TaskMergeOperation,
        expected_conflict_digest: Option<String>,
    ) -> Result<TaskMergeAppendResult, StoreTransactionFailure<TaskMergeAppendError>> {
        self.write_with_capabilities(|capabilities| {
            crate::task_merge::append_local_operation(
                capabilities.transaction,
                operation,
                expected_conflict_digest.as_deref(),
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn append_local_task_merge_operation_on_connection(
        connection: &mut Connection,
        operation: TaskMergeOperation,
        expected_conflict_digest: Option<String>,
    ) -> Result<TaskMergeAppendResult, TransactionFailure<TaskMergeAppendError>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::task_merge::append_local_operation(
                transaction,
                operation,
                expected_conflict_digest.as_deref(),
            )
        })
    }
}
