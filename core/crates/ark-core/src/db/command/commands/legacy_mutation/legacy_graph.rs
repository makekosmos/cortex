use super::super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::application::legacy_mutation::{apply_legacy_graph, LegacyGraphWrite};
use crate::db::SqliteLegacyMutationSession;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LegacyGraphWriteCommand {
    write: LegacyGraphWrite,
}

impl LegacyGraphWriteCommand {
    pub(crate) fn new(write: LegacyGraphWrite) -> Self {
        Self { write }
    }
}

impl sealed::Sealed for LegacyGraphWriteCommand {}

impl DbCommand for LegacyGraphWriteCommand {
    type Output = Vec<crate::types::SyncEntity>;

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
        apply_legacy_graph(&session, self.write).map_err(DbError::from_application_error)
    }
}
