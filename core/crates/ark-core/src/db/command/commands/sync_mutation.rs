use super::super::{CommandContext, DbError};
use ark_application::sync_storage::{OutboundProgress, OutboundProgressWrite};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CommittedFeedProgressCommand {
    write: OutboundProgressWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommittedFeedProgress {
    Outbound(OutboundProgress),
}

impl CommittedFeedProgressCommand {
    pub(crate) fn new(write: OutboundProgressWrite) -> Self {
        Self { write }
    }

    pub(in crate::db::command) fn weight(&self) -> usize {
        self.write.weight()
    }

    pub(in crate::db::command) fn execute(
        self,
        context: &CommandContext,
        writer: &mut ark_store::writer::SqliteWriter,
    ) -> Result<CommittedFeedProgress, DbError> {
        context.require_sync_peer_matches(&self.write.peer_device_id)?;
        writer
            .apply_outbound_progress_write(self.write)
            .map(CommittedFeedProgress::Outbound)
            .map_err(map_outbound_progress_failure)
    }

    #[cfg(debug_assertions)]
    pub(in crate::db::command) fn execute_on_connection(
        self,
        context: &CommandContext,
        connection: &mut rusqlite::Connection,
    ) -> Result<CommittedFeedProgress, DbError> {
        context.require_sync_peer_matches(&self.write.peer_device_id)?;
        ark_store::writer::SqliteWriter::apply_outbound_progress_write_on_connection(
            connection, self.write,
        )
        .map(CommittedFeedProgress::Outbound)
        .map_err(map_outbound_progress_test_failure)
    }
}

fn map_outbound_progress_failure(
    failure: ark_store::writer::StoreTransactionFailure<
        ark_application::sync_storage::OutboundProgressWriteError,
    >,
) -> DbError {
    use ark_store::writer::{StoreErrorKind, StoreTransactionFailure};
    match failure {
        StoreTransactionFailure::Begin(error) => match error.kind {
            StoreErrorKind::DiskFull | StoreErrorKind::Io | StoreErrorKind::Corrupt => {
                DbError::StorageUnhealthy(error.to_string())
            }
            StoreErrorKind::Other => DbError::Storage(error.to_string()),
        },
        StoreTransactionFailure::Operation(error) => DbError::Storage(error.to_string()),
        StoreTransactionFailure::Commit(error) => DbError::CommitFailed(error.to_string()),
    }
}

#[cfg(debug_assertions)]
fn map_outbound_progress_test_failure(
    failure: ark_store::writer::TransactionFailure<
        ark_application::sync_storage::OutboundProgressWriteError,
    >,
) -> DbError {
    match failure {
        ark_store::writer::TransactionFailure::Begin(error) => DbError::from_sqlite(error),
        ark_store::writer::TransactionFailure::Operation(error) => {
            DbError::Storage(error.to_string())
        }
        ark_store::writer::TransactionFailure::Commit(error) => {
            DbError::CommitFailed(error.to_string())
        }
    }
}
