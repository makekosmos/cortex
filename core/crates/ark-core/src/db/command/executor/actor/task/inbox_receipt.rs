use super::{settle, Publish, WriterTask};
use crate::db::command::CommandContext;
use crate::db::{DbError, InboxState};
use ark_application::sync_storage::{InboxReceiptWrite, InboxReceiptWriteError};
use ark_store::writer::{SqliteWriter, StoreTransactionFailure};
use tokio::sync::oneshot;

pub(in crate::db::command) struct InboxReceiptTask {
    pub context: CommandContext,
    pub write: InboxReceiptWrite,
    pub reply: oneshot::Sender<Result<InboxState, DbError>>,
}

impl WriterTask for InboxReceiptTask {
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
        let result = context
            .require_sync_peer_matches(&write.peer_device_id)
            .and_then(|()| writer.receive_inbox_receipt(write).map_err(map_failure));
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) fn map_inbox_receipt_error(error: InboxReceiptWriteError) -> DbError {
    match error {
        InboxReceiptWriteError::PolicyChanged => {
            DbError::Permission("stale_or_unnegotiated_sync_policy".into())
        }
        InboxReceiptWriteError::PeerNotAdmitted => {
            DbError::Permission("sync_peer_not_admitted".into())
        }
        InboxReceiptWriteError::PeerRemoved => DbError::Permission("sync_peer_removed".into()),
        InboxReceiptWriteError::PolicyStorage(error)
        | InboxReceiptWriteError::RegistryStorage(error)
        | InboxReceiptWriteError::RemovedPeersStorage(error)
        | InboxReceiptWriteError::Inbox(error) => DbError::from_application_error(error),
    }
}

fn map_failure(failure: StoreTransactionFailure<InboxReceiptWriteError>) -> DbError {
    match failure {
        StoreTransactionFailure::Begin(error) => match error.kind {
            ark_store::writer::StoreErrorKind::DiskFull
            | ark_store::writer::StoreErrorKind::Io
            | ark_store::writer::StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            ark_store::writer::StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
        StoreTransactionFailure::Operation(error) => map_inbox_receipt_error(error),
    }
}
