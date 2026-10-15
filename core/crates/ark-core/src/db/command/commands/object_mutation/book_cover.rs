use super::super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::application::object_mutation::{apply_book_cover, LocalBookCoverWrite};
use crate::db::SqliteObjectMutationSession;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LocalBookCoverCommand {
    write: LocalBookCoverWrite,
}

impl LocalBookCoverCommand {
    pub(crate) fn new(write: LocalBookCoverWrite) -> Self {
        Self { write }
    }
}

impl sealed::Sealed for LocalBookCoverCommand {}

impl DbCommand for LocalBookCoverCommand {
    type Output = bool;

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
        let session = SqliteObjectMutationSession::new(transaction);
        apply_book_cover(&session, self.write).map_err(DbError::from_application_error)
    }
}
