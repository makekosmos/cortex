use super::super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::application::legacy_mutation::{
    apply_tracked_app_delete, apply_tracked_app_upsert, TrackedAppDelete, TrackedAppUpsert,
};
use crate::db::SqliteLegacyMutationSession;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TrackedAppUpsertCommand {
    write: TrackedAppUpsert,
}

impl TrackedAppUpsertCommand {
    pub(crate) fn new(write: TrackedAppUpsert) -> Self {
        Self { write }
    }
}

impl sealed::Sealed for TrackedAppUpsertCommand {}

impl DbCommand for TrackedAppUpsertCommand {
    type Output = crate::types::SyncEntity;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let session = SqliteLegacyMutationSession::new(transaction);
        apply_tracked_app_upsert(&session, self.write).map_err(DbError::from_application_error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TrackedAppDeleteCommand {
    write: TrackedAppDelete,
}

impl TrackedAppDeleteCommand {
    pub(crate) fn new(write: TrackedAppDelete) -> Self {
        Self { write }
    }
}

impl sealed::Sealed for TrackedAppDeleteCommand {}

impl DbCommand for TrackedAppDeleteCommand {
    type Output = crate::types::SyncEntity;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let session = SqliteLegacyMutationSession::new(transaction);
        apply_tracked_app_delete(&session, self.write).map_err(DbError::from_application_error)
    }
}
