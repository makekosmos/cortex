use super::*;

pub(in crate::db::command) struct UsageSequenceMigrationTask {
    pub context: CommandContext,
    pub device_id: String,
    pub reply: oneshot::Sender<Result<(), DbError>>,
}

impl WriterTask for UsageSequenceMigrationTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            context,
            device_id,
            reply,
        } = *self;
        let result = context.require_engine().and_then(|()| {
            writer
                .ensure_usage_sequence_migrated(&device_id)
                .map_err(|failure| match failure {
                    TransactionFailure::Begin(error) => DbError::from_sqlite(error),
                    TransactionFailure::Operation(error) => DbError::Storage(error),
                    TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
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
