use super::*;

pub(in crate::db::command) struct InboundEntityTask {
    pub entity: crate::types::SyncEntity,
    pub reply: oneshot::Sender<Result<ark_store::writer::InboundEntityOutcome, DbError>>,
}

impl WriterTask for InboundEntityTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { entity, reply } = *self;
        let result = writer.apply_inbound_entity(entity).map_err(map_failure);
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

fn map_failure(failure: ark_store::writer::StoreTransactionFailure<String>) -> DbError {
    match failure {
        ark_store::writer::StoreTransactionFailure::Begin(error) => match error.kind {
            ark_store::writer::StoreErrorKind::DiskFull
            | ark_store::writer::StoreErrorKind::Io
            | ark_store::writer::StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            ark_store::writer::StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        ark_store::writer::StoreTransactionFailure::Operation(error) => {
            DbError::from_application_error(error)
        }
        ark_store::writer::StoreTransactionFailure::Commit(error) => {
            DbError::CommitFailed(error.to_string())
        }
    }
}
