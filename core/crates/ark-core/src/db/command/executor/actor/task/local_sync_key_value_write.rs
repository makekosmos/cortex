use super::*;

pub(in crate::db::command) struct LocalSyncKeyValueWriteTask {
    pub context: CommandContext,
    pub key: String,
    pub value: String,
    pub reply: oneshot::Sender<Result<(), DbError>>,
}

impl WriterTask for LocalSyncKeyValueWriteTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            context,
            key,
            value,
            reply,
        } = *self;
        let result = context.require_engine().and_then(|()| {
            writer
                .set_local_sync_key_value(&key, &value)
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
