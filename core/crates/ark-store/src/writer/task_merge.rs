use super::WriteCapabilities;
#[cfg(debug_assertions)]
use super::{Connection, TransactionFailure};
use super::{SqliteWriter, StoreTransactionFailure};
use crate::task_merge::TaskMergeAppendError;
use ark_application::sync_policy::PolicyIdentity;
use ark_application::sync_storage::{TaskMergeBatch, TaskMergeBatchAck};
use ark_application::task_merge::TaskMergeAppendResult;
use ark_domain::task_merge::TaskMergeOperation;

pub struct TaskMergeWrites<'scope, 'connection> {
    transaction: &'scope rusqlite::Transaction<'connection>,
}

impl WriteCapabilities<'_, '_> {
    pub fn task_merge(&mut self) -> TaskMergeWrites<'_, '_> {
        TaskMergeWrites {
            transaction: self.transaction,
        }
    }
}

impl TaskMergeWrites<'_, '_> {
    pub fn prepare_outbound_batch(
        &mut self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
        limit: usize,
    ) -> Result<Option<TaskMergeBatch>, crate::task_merge::TaskMergeOutboundError> {
        crate::task_merge::prepare_task_merge_outbound_batch(
            self.transaction,
            peer_device_id,
            identity,
            limit,
        )
    }

    pub fn acknowledge_outbound_batch(
        &mut self,
        peer_device_id: &str,
        identity: &PolicyIdentity,
        batch_id: &str,
        through_position: i64,
    ) -> Result<i64, crate::task_merge::TaskMergeOutboundError> {
        crate::task_merge::acknowledge_task_merge_outbound_batch(
            self.transaction,
            peer_device_id,
            identity,
            batch_id,
            through_position,
        )
    }
}

impl SqliteWriter {
    pub fn prepare_outbound_task_merge_batch(
        &mut self,
        peer_device_id: &str,
        identity: PolicyIdentity,
        limit: usize,
    ) -> Result<
        Option<TaskMergeBatch>,
        StoreTransactionFailure<crate::task_merge::TaskMergeOutboundError>,
    > {
        self.write_with_capabilities(|capabilities| {
            capabilities
                .task_merge()
                .prepare_outbound_batch(peer_device_id, &identity, limit)
        })
    }

    #[cfg(debug_assertions)]
    pub fn prepare_outbound_task_merge_batch_on_connection(
        connection: &mut super::Connection,
        peer_device_id: &str,
        identity: PolicyIdentity,
        limit: usize,
    ) -> Result<
        Option<TaskMergeBatch>,
        StoreTransactionFailure<crate::task_merge::TaskMergeOutboundError>,
    > {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            capabilities
                .task_merge()
                .prepare_outbound_batch(peer_device_id, &identity, limit)
        })
    }

    pub fn acknowledge_outbound_task_merge_batch(
        &mut self,
        peer_device_id: &str,
        identity: PolicyIdentity,
        batch_id: &str,
        through_position: i64,
    ) -> Result<i64, StoreTransactionFailure<crate::task_merge::TaskMergeOutboundError>> {
        self.write_with_capabilities(|capabilities| {
            capabilities.task_merge().acknowledge_outbound_batch(
                peer_device_id,
                &identity,
                batch_id,
                through_position,
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn acknowledge_outbound_task_merge_batch_on_connection(
        connection: &mut super::Connection,
        peer_device_id: &str,
        identity: PolicyIdentity,
        batch_id: &str,
        through_position: i64,
    ) -> Result<i64, StoreTransactionFailure<crate::task_merge::TaskMergeOutboundError>> {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            capabilities.task_merge().acknowledge_outbound_batch(
                peer_device_id,
                &identity,
                batch_id,
                through_position,
            )
        })
    }

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
