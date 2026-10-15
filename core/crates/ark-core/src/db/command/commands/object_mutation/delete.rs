use super::super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::application::object_mutation::{
    apply_object_delete, LocalObjectDelete, ObjectDeleteCommit,
};
use crate::db::SqliteObjectMutationSession;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LocalObjectDeleteCommand {
    write: LocalObjectDelete,
}

impl LocalObjectDeleteCommand {
    pub(crate) fn new(write: LocalObjectDelete) -> Self {
        Self { write }
    }
}

impl sealed::Sealed for LocalObjectDeleteCommand {}

impl DbCommand for LocalObjectDeleteCommand {
    type Output = ObjectDeleteCommit;

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
        apply_object_delete(&session, self.write).map_err(DbError::from_application_error)
    }
}
