use super::*;
use ::domain::sync_storage::InboxReceiptWrite;

pub(in crate::db::command) struct AuthenticatedInboundEntityTask {
    pub peer_id: String,
    pub write: InboxReceiptWrite,
    pub entity: crate::types::SyncEntity,
    pub reply: oneshot::Sender<Result<::store::writer::AuthenticatedInboundEntityOutcome, DbError>>,
}

impl WriterTask for AuthenticatedInboundEntityTask {
    fn run(
        self: Box<Self>,
        writer: &mut SqliteWriter,
        on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self {
            peer_id,
            write,
            entity,
            reply,
        } = *self;
        let result = if write.peer_device_id != peer_id {
            Err(DbError::Permission(
                "authenticated_sync_peer_mismatch".into(),
            ))
        } else {
            writer
                .apply_authenticated_inbound_entity(write, entity)
                .map_err(map_failure)
        };
        settle(result, reply, on_error)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

fn map_failure(
    failure: ::store::writer::StoreTransactionFailure<
        ::store::writer::AuthenticatedInboundEntityError,
    >,
) -> DbError {
    use ::store::writer::{AuthenticatedInboundEntityError as Error, StoreTransactionFailure};
    match failure {
        StoreTransactionFailure::Begin(error) => match error.kind {
            ::store::writer::StoreErrorKind::DiskFull
            | ::store::writer::StoreErrorKind::Io
            | ::store::writer::StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            ::store::writer::StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
        StoreTransactionFailure::Operation(Error::Mutation(error)) => {
            DbError::from_application_error(error)
        }
        StoreTransactionFailure::Operation(Error::Receipt(error)) => match error {
            ::domain::sync_storage::InboxReceiptWriteError::PolicyChanged => {
                DbError::Permission("stale_or_unnegotiated_sync_policy".into())
            }
            ::domain::sync_storage::InboxReceiptWriteError::PeerNotAdmitted => {
                DbError::Permission("sync_peer_not_admitted".into())
            }
            ::domain::sync_storage::InboxReceiptWriteError::PeerRemoved => {
                DbError::Permission("sync_peer_removed".into())
            }
            ::domain::sync_storage::InboxReceiptWriteError::PolicyStorage(error)
            | ::domain::sync_storage::InboxReceiptWriteError::RegistryStorage(error)
            | ::domain::sync_storage::InboxReceiptWriteError::RemovedPeersStorage(error)
            | ::domain::sync_storage::InboxReceiptWriteError::Inbox(error) => {
                DbError::from_application_error(error)
            }
        },
    }
}
