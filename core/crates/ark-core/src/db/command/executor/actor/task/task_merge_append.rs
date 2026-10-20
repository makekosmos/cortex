use super::{settle, Publish, WriterTask};
use crate::db::command::CommandContext;
use crate::db::DbError;
use ark_application::task_merge::TaskMergeAppendResult;
use ark_domain::task_merge::TaskMergeOperation;
use ark_store::writer::SqliteWriter;
use tokio::sync::oneshot;

pub(in crate::db::command) struct AppendTaskMergeOperationTask {
    pub context: CommandContext,
    pub operation: TaskMergeOperation,
    pub expected_conflict_digest: Option<String>,
    pub reply: oneshot::Sender<Result<TaskMergeAppendResult, DbError>>,
}

impl WriterTask for AppendTaskMergeOperationTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            context,
            operation,
            expected_conflict_digest,
            reply,
        } = *self;
        let result = context.require_engine().and_then(|()| {
            writer
                .append_local_task_merge_operation(operation, expected_conflict_digest)
                .map_err(map_append_failure)
        });
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) fn map_append_failure(
    failure: ark_store::writer::StoreTransactionFailure<
        ark_store::task_merge::TaskMergeAppendError,
    >,
) -> DbError {
    use ark_store::writer::{StoreErrorKind, StoreTransactionFailure};
    match failure {
        StoreTransactionFailure::Begin(error) => match error.kind {
            StoreErrorKind::DiskFull | StoreErrorKind::Io | StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        StoreTransactionFailure::Operation(error) => map_append_operation_error(error),
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}

pub(in crate::db::command) fn map_append_operation_error(
    error: ark_store::task_merge::TaskMergeAppendError,
) -> DbError {
    use ark_store::task_merge::TaskMergeAppendError;
    match error {
        TaskMergeAppendError::OperationIdCollision
        | TaskMergeAppendError::ConflictSetChanged
        | TaskMergeAppendError::ConflictDigestRequired => DbError::Conflict(error.to_string()),
        TaskMergeAppendError::Invalid(error) => DbError::Invalid(error),
        TaskMergeAppendError::Storage(error) => DbError::Storage(error),
    }
}
