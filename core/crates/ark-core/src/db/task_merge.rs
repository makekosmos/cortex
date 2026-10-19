use ark_domain::task_merge::{TaskConflictSet, TaskMergeOperation, TaskMergeUnit};
use rusqlite::{Connection, Transaction};
use serde::Serialize;

#[path = "task_merge/bootstrap.rs"]
mod bootstrap;
#[path = "task_merge/inbound.rs"]
mod inbound;
#[path = "task_merge/progress.rs"]
mod progress;
use bootstrap::append_bootstrap;
pub(crate) use inbound::apply_inbound_batch;
pub(crate) use progress::{acknowledge_outbound_batch, prepare_outbound_batch};

#[derive(Debug, thiserror::Error)]
pub(crate) enum TaskMergeError {
    #[error("task_merge_operation_id_collision")]
    OperationIdCollision,
    #[error("task_merge_conflict_set_changed")]
    ConflictSetChanged,
    #[error("task_merge_conflict_digest_required")]
    ConflictDigestRequired,
    #[error("{0}")]
    Permission(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Storage(String),
}

impl From<ark_store::task_merge::TaskMergeStoreError> for TaskMergeError {
    fn from(error: ark_store::task_merge::TaskMergeStoreError) -> Self {
        match error {
            ark_store::task_merge::TaskMergeStoreError::Invalid(error) => Self::Invalid(error),
            ark_store::task_merge::TaskMergeStoreError::Storage(error) => Self::Storage(error),
            ark_store::task_merge::TaskMergeStoreError::OperationIdCollision => {
                Self::OperationIdCollision
            }
        }
    }
}

pub(crate) fn require_delivery_policy(
    connection: &Connection,
    peer_device_id: &str,
    expected_identity: &ark_application::sync_storage::PolicyIdentity,
) -> Result<(), TaskMergeError> {
    let policy =
        ark_store::data_platform::load_accepted_peer_sync_policy(connection, peer_device_id)
            .map_err(TaskMergeError::Storage)?;
    let identity = policy.as_ref().map_or_else(
        ark_application::sync_policy::default_sync_policy_identity,
        |policy| policy.identity(),
    );
    if identity != *expected_identity {
        return Err(TaskMergeError::Permission(
            "stale_or_unnegotiated_sync_policy".into(),
        ));
    }
    if let Some(policy) = policy.filter(|policy| policy.active) {
        let profile = policy.selective_profile();
        let Some(rule) = profile.rules.get(&("type".into(), "core.task".into())) else {
            return Err(TaskMergeError::Permission(
                "task_merge_policy_requires_full_type_sync".into(),
            ));
        };
        if rule.mode != ark_domain::selective_sync::SyncMode::Full {
            return Err(TaskMergeError::Permission(
                "task_merge_policy_requires_full_type_sync".into(),
            ));
        }
        if rule.filter.digest() != ark_domain::selective_sync::FilterV1::match_all().digest() {
            return Err(TaskMergeError::Permission(
                "task_merge_policy_filter_unsupported".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskMergeAppendResult {
    pub conflict_set: Option<TaskConflictSet>,
    pub dependencies_pending: bool,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskMergeReadResult {
    pub conflict_set: Option<TaskConflictSet>,
    pub dependencies_pending: bool,
}

pub(crate) fn append_operation(
    transaction: &Transaction<'_>,
    operation: TaskMergeOperation,
    expected_conflict_digest: Option<&str>,
) -> Result<TaskMergeAppendResult, TaskMergeError> {
    append_operation_inner(
        transaction,
        operation,
        expected_conflict_digest,
        ark_store::task_merge::TaskMergeOperationOrigin::Local,
    )
}

pub(crate) fn append_remote_operation(
    transaction: &Transaction<'_>,
    operation: TaskMergeOperation,
    expected_conflict_digest: Option<&str>,
) -> Result<TaskMergeAppendResult, TaskMergeError> {
    append_operation_inner(
        transaction,
        operation,
        expected_conflict_digest,
        ark_store::task_merge::TaskMergeOperationOrigin::Remote,
    )
}

fn append_operation_inner(
    transaction: &Connection,
    operation: TaskMergeOperation,
    expected_conflict_digest: Option<&str>,
    origin: ark_store::task_merge::TaskMergeOperationOrigin,
) -> Result<TaskMergeAppendResult, TaskMergeError> {
    let prepared = match ark_store::task_merge::check_operation(
        transaction,
        &operation,
        expected_conflict_digest,
        origin,
    )? {
        ark_store::task_merge::TaskMergeOperationCheck::Replay(read) => {
            return Ok(TaskMergeAppendResult {
                conflict_set: read.conflict_set,
                dependencies_pending: read.dependencies_pending,
                replayed: true,
            });
        }
        ark_store::task_merge::TaskMergeOperationCheck::New(prepared) => prepared,
    };
    let operation = prepared.operation().clone();

    let (current, dependencies_pending) =
        read_conflict_set(transaction, &operation.object_id, operation.unit)?;
    let resolves_conflict = current.as_ref().is_some_and(|set| {
        set.has_conflict()
            && set
                .variants
                .iter()
                .all(|variant| operation.parents.contains(&variant.id))
    });
    if origin == ark_store::task_merge::TaskMergeOperationOrigin::Local
        && resolves_conflict
        && expected_conflict_digest.is_none()
    {
        return Err(TaskMergeError::ConflictDigestRequired);
    }
    if origin == ark_store::task_merge::TaskMergeOperationOrigin::Local {
        if let Some(expected) = expected_conflict_digest {
            if dependencies_pending
                || current.as_ref().map(|set| set.digest.as_str()) != Some(expected)
            {
                return Err(TaskMergeError::ConflictSetChanged);
            }
        }
    }

    ark_store::task_merge::persist_operation(transaction, prepared)?;

    ark_store::task_merge::materialize_task_if_ready(transaction, &operation.object_id)?;

    let (conflict_set, dependencies_pending) =
        read_conflict_set(transaction, &operation.object_id, operation.unit)?;
    Ok(TaskMergeAppendResult {
        conflict_set,
        dependencies_pending: dependencies_pending
            || task_causal_dependencies_pending(transaction, &operation.object_id)?,
        replayed: false,
    })
}

pub(crate) fn read_conflict_set(
    connection: &Connection,
    object_id: &str,
    unit: TaskMergeUnit,
) -> Result<(Option<TaskConflictSet>, bool), TaskMergeError> {
    let read = ark_store::task_merge::read_conflict_set(connection, object_id, unit)?;
    Ok((read.conflict_set, read.dependencies_pending))
}

pub(crate) fn task_causal_dependencies_pending(
    connection: &Connection,
    object_id: &str,
) -> Result<bool, TaskMergeError> {
    ark_store::task_merge::task_causal_dependencies_pending(connection, object_id)
        .map_err(Into::into)
}

pub(crate) fn read_conflict_set_result(
    connection: &Connection,
    object_id: &str,
    unit: TaskMergeUnit,
) -> Result<TaskMergeReadResult, TaskMergeError> {
    let (conflict_set, dependencies_pending) = read_conflict_set(connection, object_id, unit)?;
    Ok(TaskMergeReadResult {
        conflict_set,
        dependencies_pending,
    })
}
