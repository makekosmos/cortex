use ark_domain::task_merge::{TaskConflictSet, TaskMergeOperation, TaskMergeUnit};
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[path = "task_merge/bootstrap.rs"]
mod bootstrap;
#[path = "task_merge/inbound.rs"]
mod inbound;
#[path = "task_merge/mutation.rs"]
mod mutation;
#[path = "task_merge/progress.rs"]
mod progress;
use bootstrap::{append_bootstrap, append_local_bootstrap, materialize_if_ready};
pub(crate) use inbound::apply_inbound_batch;
pub(crate) use mutation::{commit_local_task_delete, commit_local_task_upsert};
pub(crate) use progress::{acknowledge_outbound_batch, prepare_outbound_batch};

pub(crate) fn object_snapshot_token(
    connection: &Connection,
    object_id: &str,
) -> Result<Option<String>, TaskMergeError> {
    let bootstrapped = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM task_merge_bootstraps WHERE object_id = ?1)",
            [object_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    if !bootstrapped {
        return Ok(None);
    }

    let mut heads = Vec::with_capacity(TaskMergeUnit::ALL.len());
    for unit in TaskMergeUnit::ALL {
        let (set, dependencies_pending) = read_conflict_set(connection, object_id, unit)?;
        if dependencies_pending {
            return Err(TaskMergeError::Invalid(
                "task_merge_dependencies_pending".into(),
            ));
        }
        let set =
            set.ok_or_else(|| TaskMergeError::Invalid("task_merge_bootstrap_incomplete".into()))?;
        heads.push((unit, set.digest));
    }
    let encoded =
        serde_json::to_vec(&heads).map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    let mut digest = Sha256::new();
    digest.update(b"ark-task-snapshot-v1\0");
    digest.update(encoded);
    Ok(Some(format!("task-merge:sha256:{:x}", digest.finalize())))
}

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
        }
    }
}

#[derive(Clone, Copy)]
enum DeliveryTiming {
    Immediate,
    AfterBootstrap,
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
        true,
        false,
        DeliveryTiming::Immediate,
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
        false,
        true,
        DeliveryTiming::AfterBootstrap,
    )
}

fn append_operation_inner(
    transaction: &Connection,
    operation: TaskMergeOperation,
    expected_conflict_digest: Option<&str>,
    check_resolution_precondition: bool,
    check_replayed_guard: bool,
    delivery_timing: DeliveryTiming,
) -> Result<TaskMergeAppendResult, TaskMergeError> {
    let (operation, operation_json, payload_sha256) =
        ark_store::task_merge::canonical_operation(operation)?;
    if let Some((stored_hash,)) = transaction
        .query_row(
            "SELECT payload_sha256 FROM task_merge_operations WHERE operation_id = ?1",
            [operation.id.as_str()],
            |row| Ok((row.get::<_, String>(0)?,)),
        )
        .optional()
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?
    {
        if stored_hash != payload_sha256 {
            return Err(TaskMergeError::OperationIdCollision);
        }
        if check_replayed_guard {
            let stored_guard = transaction
                .query_row(
                    "SELECT expected_conflict_digest
                     FROM task_merge_resolution_guards WHERE operation_id = ?1",
                    [operation.id.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
            if stored_guard.as_deref() != expected_conflict_digest {
                return Err(TaskMergeError::OperationIdCollision);
            }
        }
        let (conflict_set, dependencies_pending) =
            read_conflict_set(transaction, &operation.object_id, operation.unit)?;
        return Ok(TaskMergeAppendResult {
            conflict_set,
            dependencies_pending,
            replayed: true,
        });
    }

    let (current, dependencies_pending) =
        read_conflict_set(transaction, &operation.object_id, operation.unit)?;
    let resolves_conflict = current.as_ref().is_some_and(|set| {
        set.has_conflict()
            && set
                .variants
                .iter()
                .all(|variant| operation.parents.contains(&variant.id))
    });
    if check_resolution_precondition && resolves_conflict && expected_conflict_digest.is_none() {
        return Err(TaskMergeError::ConflictDigestRequired);
    }
    if check_resolution_precondition {
        if let Some(expected) = expected_conflict_digest {
            if dependencies_pending
                || current.as_ref().map(|set| set.digest.as_str()) != Some(expected)
            {
                return Err(TaskMergeError::ConflictSetChanged);
            }
        }
    }

    let unit = serde_json::to_string(&operation.unit)
        .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    transaction
        .execute(
            "INSERT INTO task_merge_operations(
                operation_id, object_id, unit, operation_json, payload_sha256
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                operation.id.as_str(),
                operation.object_id,
                unit,
                operation_json,
                payload_sha256,
            ],
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    if let Some(expected_digest) = expected_conflict_digest {
        transaction
            .execute(
                "INSERT INTO task_merge_resolution_guards(
                    operation_id, expected_conflict_digest
                 ) VALUES (?1, ?2)",
                rusqlite::params![operation.id.as_str(), expected_digest],
            )
            .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    }
    let has_bootstrap = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM task_merge_bootstraps WHERE object_id = ?1)",
            [&operation.object_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    if has_bootstrap || matches!(delivery_timing, DeliveryTiming::Immediate) {
        transaction
            .execute(
                "INSERT INTO task_merge_journal(record_kind, record_id)
                 VALUES ('operation', ?1)",
                [operation.id.as_str()],
            )
            .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    }

    materialize_if_ready(transaction, &operation.object_id)?;

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
