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
        if crate::db::get_object(transaction, &self.write.id)
            .map_err(|error| DbError::Storage(error.to_string()))?
            .is_some_and(|object| object.type_id == "core.task")
        {
            let session = crate::db::SqliteObjectWriteSession::new(transaction);
            let commit = crate::application::task_mutation::apply_task_delete(
                &session,
                crate::application::task_mutation::LocalTaskDelete {
                    id: self.write.id,
                    expected_snapshot: self.write.expected_snapshot,
                    device_id: self.write.device_id,
                    mode: crate::application::task_mutation::TaskDeleteMode::Hard,
                },
            )
            .map_err(DbError::from_application_error)?;
            return Ok(ObjectDeleteCommit {
                entity: commit.entity,
                id: commit.id,
            });
        }
        let session = SqliteObjectMutationSession::new(transaction);
        apply_object_delete(&session, self.write).map_err(DbError::from_application_error)
    }
}
