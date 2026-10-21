use super::task::{settle, Publish, WriterTask};
use crate::db::command::commands::{
    AcknowledgeTaskMergeBatchCommand, PrepareTaskMergeBatchCommand,
};
use crate::db::DbError;
use ark_application::sync_storage::TaskMergeBatch;
use ark_store::task_merge::TaskMergeOutboundError;
use ark_store::writer::{SqliteWriter, StoreErrorKind, StoreTransactionFailure};
use tokio::sync::oneshot;

pub(in crate::db::command) struct PrepareTaskMergeBatchTask {
    pub command: PrepareTaskMergeBatchCommand,
    pub reply: oneshot::Sender<Result<Option<TaskMergeBatch>, DbError>>,
}

impl WriterTask for PrepareTaskMergeBatchTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { command, reply } = *self;
        let result = writer
            .prepare_outbound_task_merge_batch(
                &command.peer_id,
                command.expected_identity,
                command.limit,
            )
            .map_err(map_task_merge_progress_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) struct AcknowledgeTaskMergeBatchTask {
    pub command: AcknowledgeTaskMergeBatchCommand,
    pub reply: oneshot::Sender<Result<i64, DbError>>,
}

impl WriterTask for AcknowledgeTaskMergeBatchTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { command, reply } = *self;
        let result = writer
            .acknowledge_outbound_task_merge_batch(
                &command.peer_id,
                command.expected_identity,
                &command.batch_id,
                command.through_position,
            )
            .map_err(map_task_merge_progress_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) fn map_task_merge_progress_failure(
    failure: StoreTransactionFailure<TaskMergeOutboundError>,
) -> DbError {
    match failure {
        StoreTransactionFailure::Begin(error) => match error.kind {
            StoreErrorKind::DiskFull | StoreErrorKind::Io | StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        StoreTransactionFailure::Operation(error) => match error {
            TaskMergeOutboundError::Permission(error) => DbError::Permission(error),
            TaskMergeOutboundError::Invalid(error) => DbError::Invalid(error),
            TaskMergeOutboundError::Storage(error) => DbError::Storage(error),
        },
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}
