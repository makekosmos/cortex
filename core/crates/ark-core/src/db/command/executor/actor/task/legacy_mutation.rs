use super::task::{settle, Publish, WriterTask};
use crate::application::legacy_mutation::LegacyDelete;
use crate::db::command::{DbError, LegacyDeleteCommand, LegacyGraphWriteCommand};
use ark_store::writer::{SqliteWriter, TransactionFailure};
use tokio::sync::oneshot;

pub(in crate::db::command) struct LegacyGraphWriteTask {
    pub command: LegacyGraphWriteCommand,
    pub reply: oneshot::Sender<Result<Vec<crate::types::SyncEntity>, DbError>>,
}

impl WriterTask for LegacyGraphWriteTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let crate::application::legacy_mutation::LegacyGraphWrite { records, device_id } =
            self.command.write;
        let result = writer
            .apply_local_legacy_graph_write(&records, "rpc", device_id)
            .map_err(map_legacy_transaction_failure);
        settle(result, self.reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) struct LegacyDeleteTask {
    pub command: LegacyDeleteCommand,
    pub reply: oneshot::Sender<Result<crate::types::SyncEntity, DbError>>,
}

impl WriterTask for LegacyDeleteTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let LegacyDelete {
            id,
            kind,
            device_id,
        } = self.command.write;
        let result = writer
            .apply_local_legacy_delete(&id, kind.type_id(), device_id)
            .map_err(map_legacy_transaction_failure);
        settle(result, self.reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) fn map_legacy_transaction_failure(
    failure: TransactionFailure<String>,
) -> DbError {
    match failure {
        TransactionFailure::Begin(error) => DbError::from_store_error(error),
        TransactionFailure::Operation(error) => DbError::from_application_error(error),
        TransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}
