use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError, DbQuery};
use crate::application::sync_storage::{PolicyBoundTaskMergeJournalPage, PolicyIdentity};
use crate::db::task_merge::{
    append_operation, read_conflict_set_result, read_journal_page, TaskMergeAppendResult,
    TaskMergeError, TaskMergeReadResult,
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

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TaskMergeJournalPageQuery {
    peer_id: String,
    expected_identity: PolicyIdentity,
    after_position: i64,
    through_position: i64,
    limit: usize,
}

impl TaskMergeJournalPageQuery {
    pub(crate) fn new(
        peer_id: String,
        expected_identity: PolicyIdentity,
        after_position: i64,
        through_position: i64,
        limit: usize,
    ) -> Self {
        Self {
            peer_id,
            expected_identity,
            after_position,
            through_position,
            limit,
        }
    }
}

impl sealed::Sealed for TaskMergeJournalPageQuery {}

impl DbQuery for TaskMergeJournalPageQuery {
    type Output = PolicyBoundTaskMergeJournalPage;

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &rusqlite::Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_id)?;
        let policy =
            crate::data_platform::load_accepted_peer_sync_policy(connection, &self.peer_id)
                .map_err(DbError::from_application_error)?;
        let identity = policy.as_ref().map_or_else(
            crate::data_platform::default_sync_policy_identity,
            |policy| policy.identity(),
        );
        if identity != self.expected_identity {
            return Err(DbError::Permission(
                "stale_or_unnegotiated_sync_policy".into(),
            ));
        }
        if let Some(policy) = policy.filter(|policy| policy.active) {
            if policy.selective_profile().rule("type", "core.task")
                != crate::data_platform::SyncMode::Full
            {
                return Err(DbError::Permission(
                    "task_merge_policy_requires_full_type_sync".into(),
                ));
            }
        }
        let page = read_journal_page(
            connection,
            self.after_position,
            self.through_position,
            self.limit,
        )
        .map_err(map_task_merge_error)?;
        Ok(PolicyBoundTaskMergeJournalPage { identity, page })
    }
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
