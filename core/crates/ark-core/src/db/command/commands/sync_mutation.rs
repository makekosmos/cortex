use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::db::SqliteStorageBackend;
use crate::types::SyncEntity;
use ark_application::sync_storage::ReplayNotice;
use ark_application::sync_storage::{OutboundProgress, OutboundProgressWrite};
use rusqlite::Transaction;
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

struct SqliteInboundSyncEntitySession<'transaction, 'connection>(
    &'transaction Transaction<'connection>,
);

impl ark_application::sync_storage::SyncEntityMutationSession
    for SqliteInboundSyncEntitySession<'_, '_>
{
    fn apply_entity(
        &mut self,
        entity: &SyncEntity,
        authenticated_peer_id: Option<&str>,
    ) -> Result<
        (
            ark_application::sync_storage::InboundApplyOutcome,
            Vec<ReplayNotice>,
        ),
        String,
    > {
        match authenticated_peer_id {
            Some(peer_id) => SqliteStorageBackend::apply_authenticated_entity_in_transaction(
                self.0, entity, peer_id,
            ),
            None => SqliteStorageBackend::apply_entity_in_transaction(self.0, entity),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApplySyncEntityCommand {
    entity: SyncEntity,
}

impl ApplySyncEntityCommand {
    pub(crate) fn new(entity: SyncEntity) -> Self {
        Self { entity }
    }
}

impl sealed::Sealed for ApplySyncEntityCommand {}

impl DbCommand for ApplySyncEntityCommand {
    type Output = Vec<ReplayNotice>;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        let mut session = SqliteInboundSyncEntitySession(transaction);
        let (_, notices) = ark_application::sync_storage::apply_inbound_sync_entity(
            &mut session,
            &self.entity,
            None,
        )
        .map_err(DbError::from_application_error)?;
        Ok(notices)
    }
}
