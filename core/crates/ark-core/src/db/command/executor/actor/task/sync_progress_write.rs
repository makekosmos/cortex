use super::*;

pub(in crate::db::command) struct SyncProgressWriteTask {
    pub context: CommandContext,
    pub write: ark_application::sync_storage::SyncProgressWrite,
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
                ark_store::writer::StoreTransactionFailure::Begin(error) => {
                    DbError::Storage(error.to_string())
                }
                ark_store::writer::StoreTransactionFailure::Operation(error) => match error {
                    ark_application::sync_storage::SyncProgressWriteError::MissingLocalDevice => {
                        DbError::Permission(
                            "sync_policy_identity_missing_local_device".into(),
                        )
                    }
                    ark_application::sync_storage::SyncProgressWriteError::StalePolicy => {
                        DbError::Permission("stale_or_unnegotiated_sync_policy".into())
                    }
                    ark_application::sync_storage::SyncProgressWriteError::PolicyStorage(error) => {
                        DbError::from_application_error(error)
                    }
                    ark_application::sync_storage::SyncProgressWriteError::SyncValueStorage(
                        error,
                    ) => {
                        DbError::Storage(error)
                    }
                },
                ark_store::writer::StoreTransactionFailure::Commit(error) => {
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
