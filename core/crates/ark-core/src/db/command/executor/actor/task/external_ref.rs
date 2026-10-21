use super::*;

pub(in crate::db::command) struct ExternalRefUpsertTask {
    pub request: ark_application::external_ref::ExternalRefUpsert,
    pub reply: oneshot::Sender<Result<(), DbError>>,
}

impl WriterTask for ExternalRefUpsertTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { request, reply } = *self;
        let result = writer
            .upsert_external_ref(request)
            .map_err(map_application_transaction_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}
