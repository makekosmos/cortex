use super::*;

pub(in crate::db::command) struct LocalCoreTaskWriteTask {
    pub context: CommandContext,
    pub command: LocalCoreTaskWrite,
    pub operation_id: Option<String>,
    pub reply: oneshot::Sender<Result<TaskWriteOperationResult, DbError>>,
}

impl WriterTask for LocalCoreTaskWriteTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            context,
            command,
            operation_id,
            reply,
        } = *self;
        let result = context.require_engine().and_then(|()| {
            if command.object_type_id() != "core.task" {
                return Err(DbError::Unsupported(
                    "object_write_slice_unsupported_type".into(),
                ));
            }
            let principal = context.operation_principal();
            writer
                .apply_local_core_task_write(command, &principal, operation_id.as_deref())
                .map_err(|failure| match failure {
                    ark_store::writer::StoreTransactionFailure::Begin(error) => {
                        DbError::Storage(error.to_string())
                    }
                    ark_store::writer::StoreTransactionFailure::Operation(
                        ark_store::writer::TaskObjectWriteError::OperationIdConflict,
                    ) => DbError::Conflict(
                        "operationId was already used with a different command payload".into(),
                    ),
                    ark_store::writer::StoreTransactionFailure::Operation(
                        ark_store::writer::TaskObjectWriteError::Storage(error),
                    ) => DbError::from_application_error(error),
                    ark_store::writer::StoreTransactionFailure::Commit(error) => {
                        DbError::CommitFailed(error.to_string())
                    }
                })
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
