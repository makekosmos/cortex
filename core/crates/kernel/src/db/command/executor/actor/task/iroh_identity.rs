use super::*;

pub(in crate::db::command) struct IrohIdentityKeyTask {
    pub candidate_hex: String,
    pub reply: oneshot::Sender<Result<String, DbError>>,
}

impl WriterTask for IrohIdentityKeyTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            candidate_hex,
            reply,
        } = *self;
        let result = writer
            .persist_iroh_secret_key_candidate(candidate_hex)
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
