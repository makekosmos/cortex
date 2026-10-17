use crate::domain::task_merge::{conflict_set, TaskConflictSet, TaskMergeOperation, TaskMergeUnit};
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, thiserror::Error)]
pub(crate) enum TaskMergeError {
    #[error("task_merge_operation_id_collision")]
    OperationIdCollision,
    #[error("task_merge_conflict_set_changed")]
    ConflictSetChanged,
    #[error("task_merge_conflict_digest_required")]
    ConflictDigestRequired,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Storage(String),
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
    let (operation, operation_json, payload_sha256) = canonical_operation(operation)?;
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
    if resolves_conflict && expected_conflict_digest.is_none() {
        return Err(TaskMergeError::ConflictDigestRequired);
    }
    if let Some(expected) = expected_conflict_digest {
        if dependencies_pending || current.as_ref().map(|set| set.digest.as_str()) != Some(expected)
        {
            return Err(TaskMergeError::ConflictSetChanged);
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
    transaction
        .execute(
            "INSERT INTO task_merge_journal(operation_id) VALUES (?1)",
            [operation.id.as_str()],
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;

    let (conflict_set, dependencies_pending) =
        read_conflict_set(transaction, &operation.object_id, operation.unit)?;
    Ok(TaskMergeAppendResult {
        conflict_set,
        dependencies_pending,
        replayed: false,
    })
}

pub(crate) fn read_conflict_set(
    connection: &Connection,
    object_id: &str,
    unit: TaskMergeUnit,
) -> Result<(Option<TaskConflictSet>, bool), TaskMergeError> {
    let unit_key =
        serde_json::to_string(&unit).map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    let mut statement = connection
        .prepare(
            "SELECT operation_id, operation_json FROM task_merge_operations
             WHERE object_id = ?1 AND unit = ?2 ORDER BY operation_id",
        )
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    let rows = statement
        .query_map(rusqlite::params![object_id, unit_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| TaskMergeError::Storage(error.to_string()))?;
    let operations = rows
        .map(|row| {
            let (stored_id, json) =
                row.map_err(|error| TaskMergeError::Storage(error.to_string()))?;
            let operation = serde_json::from_str::<TaskMergeOperation>(&json)
                .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
            if operation.id.as_str() != stored_id
                || operation.object_id != object_id
                || operation.unit != unit
            {
                return Err(TaskMergeError::Invalid(
                    "task_merge_operation_identity_mismatch".into(),
                ));
            }
            Ok(operation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    conflict_set_for_stored(operations)
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

fn conflict_set_for_stored(
    operations: Vec<TaskMergeOperation>,
) -> Result<(Option<TaskConflictSet>, bool), TaskMergeError> {
    if operations.is_empty() {
        return Ok((None, false));
    }
    let by_id = operations
        .iter()
        .map(|operation| (operation.id.clone(), operation))
        .collect::<BTreeMap<_, _>>();
    let mut pending = operations
        .iter()
        .filter(|operation| {
            operation
                .parents
                .iter()
                .any(|parent| !by_id.contains_key(parent))
        })
        .map(|operation| operation.id.clone())
        .collect::<BTreeSet<_>>();
    loop {
        let newly_pending = operations
            .iter()
            .filter(|operation| {
                !pending.contains(&operation.id)
                    && operation
                        .parents
                        .iter()
                        .any(|parent| pending.contains(parent))
            })
            .map(|operation| operation.id.clone())
            .collect::<Vec<_>>();
        if newly_pending.is_empty() {
            break;
        }
        pending.extend(newly_pending);
    }

    let applied = operations
        .into_iter()
        .filter(|operation| !pending.contains(&operation.id))
        .collect::<Vec<_>>();
    if applied.is_empty() {
        return Ok((None, true));
    }
    let set = conflict_set(&applied).map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    Ok((Some(set), !pending.is_empty()))
}

fn canonical_operation(
    mut operation: TaskMergeOperation,
) -> Result<(TaskMergeOperation, String, String), TaskMergeError> {
    crate::domain::task_merge::validate_operation(&operation)
        .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    operation.parents.sort();
    let value = canonical_value(
        &serde_json::to_value(&operation)
            .map_err(|error| TaskMergeError::Invalid(error.to_string()))?,
    );
    let json = serde_json::to_string(&value)
        .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    let hash = format!("{:x}", Sha256::digest(json.as_bytes()));
    Ok((operation, json, hash))
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted = object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect::<Map<_, _>>())
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_value).collect()),
        other => other.clone(),
    }
}
