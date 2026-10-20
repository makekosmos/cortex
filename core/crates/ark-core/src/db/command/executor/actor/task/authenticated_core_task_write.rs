use super::*;

pub(in crate::db::command) struct AuthenticatedCoreTaskWriteTask {
    pub peer_id: String,
    pub command: AuthenticatedCoreTaskWrite,
    pub reply: oneshot::Sender<Result<RemoteCoreTaskOutcome, DbError>>,
}

impl WriterTask for AuthenticatedCoreTaskWriteTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            peer_id,
            command,
            reply,
        } = *self;
        let result = writer
            .apply_authenticated_core_task_write(&peer_id, command)
            .map_err(|failure| {
                match failure {
                ark_store::writer::StoreTransactionFailure::Begin(error) => {
                    DbError::Storage(error.to_string())
                }
                ark_store::writer::StoreTransactionFailure::Operation(error) => {
                    crate::db::command::handle::authenticated_core_task_write::
                        map_authenticated_core_task_error(error)
                }
                ark_store::writer::StoreTransactionFailure::Commit(error) => {
                    DbError::CommitFailed(error.to_string())
                }
            }
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
