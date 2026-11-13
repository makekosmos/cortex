use super::*;
use ::store::sync_progress_write::SyncProgressWriteError;

pub(in crate::db::command) struct SyncProgressWriteTask {
    pub context: CommandContext,
    pub write: ::store::sync_progress_write::SyncProgressWrite,
    pub reply: oneshot::Sender<Result<(), DbError>>,
}

impl WriterTask for SyncProgressWriteTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            context,
            write,
            reply,
        } = *self;
        let result =
            context.require_system_sync().and_then(|()| {
                writer.set_sync_progress(write).map_err(|failure| {
                    match failure {
                ::store::writer::StoreTransactionFailure::Begin(error) => {
                    DbError::Storage(error.to_string())
                }
                ::store::writer::StoreTransactionFailure::Operation(error) => match error {
                    ::store::sync_progress_write::SyncProgressWriteError::MissingLocalDevice => {
                        DbError::Permission(
                            "sync_policy_identity_missing_local_device".into(),
                        )
                    }
                    ::store::sync_progress_write::SyncProgressWriteError::StalePolicy => {
                        DbError::Permission("stale_or_unnegotiated_sync_policy".into())
                    }
                    SyncProgressWriteError::PolicyStorage(error) => {
                        DbError::from_application_error(error)
                    }
                    ::store::sync_progress_write::SyncProgressWriteError::SyncValueStorage(
                        error,
                    ) => {
                        DbError::Storage(error)
                    }
                },
                ::store::writer::StoreTransactionFailure::Commit(error) => {
                    DbError::CommitFailed(error.to_string())
                }
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
