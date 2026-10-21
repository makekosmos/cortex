use super::*;

pub(in crate::db::command) struct TrackedAppMutationTask {
    pub mutation: ark_application::tracked_app_mutation::TrackedAppMutation,
    pub reply: oneshot::Sender<Result<crate::types::SyncEntity, DbError>>,
}

impl WriterTask for TrackedAppMutationTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { mutation, reply } = *self;
        let result = writer
            .apply_tracked_app_mutation(mutation)
            .map_err(map_store_application_transaction_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}
