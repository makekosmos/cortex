use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError, StoreReadQuery};
use crate::db::task_merge::TaskMergeError;
use ark_application::sync_storage::{PolicyIdentity, TaskMergeBatch, TaskMergeBatchAck};
use ark_domain::task_merge::{TaskMergeOperation, TaskMergeUnit};
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AppendTaskMergeOperationRequest {
    pub(crate) operation: TaskMergeOperation,
    pub(crate) expected_conflict_digest: Option<String>,
}

impl AppendTaskMergeOperationRequest {
    pub(crate) fn new(
        operation: TaskMergeOperation,
        expected_conflict_digest: Option<String>,
    ) -> Self {
        Self {
            operation,
            expected_conflict_digest,
        }
    }

    pub(crate) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PrepareTaskMergeBatchCommand {
    peer_id: String,
    expected_identity: PolicyIdentity,
    limit: usize,
}

impl PrepareTaskMergeBatchCommand {
    pub(crate) fn new(peer_id: String, expected_identity: PolicyIdentity, limit: usize) -> Self {
        Self {
            peer_id,
            expected_identity,
            limit,
        }
    }
}

impl sealed::Sealed for PrepareTaskMergeBatchCommand {}

impl DbCommand for PrepareTaskMergeBatchCommand {
    type Output = Option<TaskMergeBatch>;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_id)?;
        crate::db::task_merge::prepare_outbound_batch(
            transaction,
            &self.peer_id,
            &self.expected_identity,
            self.limit,
        )
        .map_err(map_task_merge_error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AcknowledgeTaskMergeBatchCommand {
    peer_id: String,
    expected_identity: PolicyIdentity,
    batch_id: String,
    through_position: i64,
}

impl AcknowledgeTaskMergeBatchCommand {
    pub(crate) fn new(
        peer_id: String,
        expected_identity: PolicyIdentity,
        batch_id: String,
        through_position: i64,
    ) -> Self {
        Self {
            peer_id,
            expected_identity,
            batch_id,
            through_position,
        }
    }
}

impl sealed::Sealed for AcknowledgeTaskMergeBatchCommand {}

impl DbCommand for AcknowledgeTaskMergeBatchCommand {
    type Output = i64;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_id)?;
        crate::db::task_merge::acknowledge_outbound_batch(
            transaction,
            &self.peer_id,
            &self.expected_identity,
            &self.batch_id,
            self.through_position,
        )
        .map_err(map_task_merge_error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApplyTaskMergeBatchCommand {
    peer_id: String,
    local_device_id: String,
    expected_identity: PolicyIdentity,
    batch: TaskMergeBatch,
}

impl ApplyTaskMergeBatchCommand {
    pub(crate) fn new(
        peer_id: String,
        local_device_id: String,
        expected_identity: PolicyIdentity,
        batch: TaskMergeBatch,
    ) -> Self {
        Self {
            peer_id,
            local_device_id,
            expected_identity,
            batch,
        }
    }
}

impl sealed::Sealed for ApplyTaskMergeBatchCommand {}

impl DbCommand for ApplyTaskMergeBatchCommand {
    type Output = TaskMergeBatchAck;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_id)?;
        super::sync_policy_identity::require_sync_policy_identity(
            transaction,
            Some(&self.local_device_id),
            Some(&self.expected_identity),
        )?;
        require_task_merge_receive_policy(transaction, &self.local_device_id)?;
        super::device_registry_sync::require_registry_peer_admitted(transaction, &self.peer_id)?;
        let removed: Vec<String> =
            ark_store::sync_kv::get_sync_kv(transaction, "sync.removed_peers")
                .map_err(DbError::from_application_error)?
                .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
                .transpose()
                .map_err(DbError::from_application_error)?
                .unwrap_or_default();
        if removed.iter().any(|removed_id| removed_id == &self.peer_id) {
            return Err(DbError::Permission("sync_peer_removed".into()));
        }
        crate::db::task_merge::apply_inbound_batch(
            transaction,
            &self.peer_id,
            &self.expected_identity,
            &self.batch,
        )
        .map_err(map_task_merge_error)
    }
}

fn require_task_merge_receive_policy(
    transaction: &Transaction<'_>,
    local_device_id: &str,
) -> Result<(), DbError> {
    let profile =
        ark_store::data_platform::load_sync_profile_for_sync(transaction, local_device_id)
            .map_err(DbError::from_application_error)?;
    if let Some(profile) = profile.filter(|profile| profile.active) {
        let selected = profile.selective_profile();
        let Some(rule) = selected.rules.get(&("type".into(), "core.task".into())) else {
            return Err(DbError::Permission(
                "task_merge_policy_requires_full_type_sync".into(),
            ));
        };
        if rule.mode != ark_domain::selective_sync::SyncMode::Full
            || rule.filter.digest() != ark_domain::selective_sync::FilterV1::match_all().digest()
        {
            return Err(DbError::Permission(
                "task_merge_policy_requires_full_type_sync".into(),
            ));
        }
    }
    Ok(())
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

impl StoreReadQuery for TaskConflictSetQuery {
    type Output = ark_application::task_merge::TaskMergeConflictRead;

    fn class(&self) -> CommandClass {
        CommandClass::Interactive
    }

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        capabilities
            .task_merge()
            .conflict_set(&self.object_id, self.unit)
            .map_err(|error| map_task_merge_error(error.into()))
    }
}

fn map_task_merge_error(error: TaskMergeError) -> DbError {
    match error {
        TaskMergeError::OperationIdCollision => DbError::Conflict(error.to_string()),
        TaskMergeError::Invalid(error) => DbError::Invalid(error),
        TaskMergeError::Permission(error) => DbError::Permission(error),
        TaskMergeError::Storage(error) => DbError::Storage(error),
    }
}
