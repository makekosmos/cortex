use super::task::{settle, Publish, WriterTask};
use crate::db::command::ApplyRegistryEventsCommand;
use crate::db::DbError;
use ark_store::writer::{RegistryEventApplyError, SqliteWriter, StoreTransactionFailure};
use tokio::sync::oneshot;

pub(in crate::db::command) struct ApplyRegistryEventsTask {
    pub command: ApplyRegistryEventsCommand,
    pub reply:
        oneshot::Sender<Result<ark_application::replication_storage::RegistryPageApply, DbError>>,
}

impl WriterTask for ApplyRegistryEventsTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { command, reply } = *self;
        let result = writer
            .apply_device_registry_events(&command.sender_device_id, &command.events)
            .map_err(|failure| match failure {
                StoreTransactionFailure::Begin(error) => DbError::Storage(error.to_string()),
                StoreTransactionFailure::Operation(error) => map_error(error),
                StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
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

pub(in crate::db::command) fn map_error(error: RegistryEventApplyError) -> DbError {
    match error {
        RegistryEventApplyError::Permission(message) => DbError::Permission(message),
        RegistryEventApplyError::Conflict(message) => DbError::Conflict(message),
        RegistryEventApplyError::Storage(message) => DbError::Storage(message),
    }
}
