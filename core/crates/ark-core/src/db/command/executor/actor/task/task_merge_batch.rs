use super::super::super::{CommandContext, DbError};
use super::task::{settle, Publish, WriterTask};
use crate::db::command::ApplyTaskMergeBatchCommand;
use ark_store::writer::SqliteWriter;
use tokio::sync::oneshot;

pub(in crate::db::command) struct ApplyTaskMergeBatchTask {
    pub context: CommandContext,
    pub command: ApplyTaskMergeBatchCommand,
    pub reply: oneshot::Sender<Result<ark_application::sync_storage::TaskMergeBatchAck, DbError>>,
}

impl WriterTask for ApplyTaskMergeBatchTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            context,
            command,
            reply,
        } = *self;
        let result = context
            .require_sync_peer_matches(&command.peer_id)
            .and_then(|()| {
                writer
                    .apply_inbound_task_merge_batch(
                        &command.peer_id,
                        &command.local_device_id,
                        command.expected_identity,
                        command.batch,
                    )
                    .map_err(map_failure)
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

pub(in crate::db::command) fn map_failure(
    failure: ark_store::writer::TransactionFailure<ark_store::task_merge::TaskMergeInboundError>,
) -> DbError {
    use ark_store::writer::TransactionFailure;
    match failure {
        TransactionFailure::Begin(error) => DbError::from_store_error(error),
        TransactionFailure::Operation(error) => match error {
            ark_store::task_merge::TaskMergeInboundError::Permission(error) => {
                DbError::Permission(error)
            }
            ark_store::task_merge::TaskMergeInboundError::Invalid(error) => DbError::Invalid(error),
            ark_store::task_merge::TaskMergeInboundError::Storage(error) => DbError::Storage(error),
            ark_store::task_merge::TaskMergeInboundError::OperationIdCollision => {
                DbError::Conflict("task_merge_operation_id_collision".into())
            }
        },
        TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}
