use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError, DbQuery};
use crate::db::task_merge::{
    append_operation, read_conflict_set_result, TaskMergeAppendResult, TaskMergeError,
    TaskMergeReadResult,
};
use crate::domain::task_merge::{TaskMergeOperation, TaskMergeUnit};
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AppendTaskMergeOperationCommand {
    operation: TaskMergeOperation,
    expected_conflict_digest: Option<String>,
}

impl AppendTaskMergeOperationCommand {
    pub(crate) fn new(
        operation: TaskMergeOperation,
        expected_conflict_digest: Option<String>,
    ) -> Self {
        Self {
            operation,
            expected_conflict_digest,
        }
    }
}

impl sealed::Sealed for AppendTaskMergeOperationCommand {}

impl DbCommand for AppendTaskMergeOperationCommand {
    type Output = TaskMergeAppendResult;

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
        append_operation(
            transaction,
            self.operation,
            self.expected_conflict_digest.as_deref(),
        )
        .map_err(map_task_merge_error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TaskConflictSetQuery {
    object_id: String,
    unit: TaskMergeUnit,
}

impl TaskConflictSetQuery {
    pub(crate) fn new(object_id: String, unit: TaskMergeUnit) -> Self {
        Self { object_id, unit }
    }
}

impl sealed::Sealed for TaskConflictSetQuery {}

impl DbQuery for TaskConflictSetQuery {
    type Output = TaskMergeReadResult;

    fn class(&self) -> CommandClass {
        CommandClass::Interactive
    }

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &rusqlite::Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        read_conflict_set_result(connection, &self.object_id, self.unit)
            .map_err(map_task_merge_error)
    }
}

fn map_task_merge_error(error: TaskMergeError) -> DbError {
    match error {
        TaskMergeError::OperationIdCollision
        | TaskMergeError::ConflictSetChanged
        | TaskMergeError::ConflictDigestRequired => DbError::Conflict(error.to_string()),
        TaskMergeError::Invalid(error) => DbError::Invalid(error),
        TaskMergeError::Storage(error) => DbError::Storage(error),
    }
}
