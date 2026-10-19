use ark_application::sync_storage::{
    TaskMergeJournalEntry, TaskMergeJournalPage, TaskMergeJournalRecord,
};
use ark_domain::task_merge::{
    conflict_set, TaskConflictSet, TaskMergeBootstrap, TaskMergeOperation, TaskMergeUnit,
};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_TASK_MERGE_PAGE_SIZE: usize = 100;

#[derive(Debug)]
pub enum TaskMergeStoreError {
    Invalid(String),
    Storage(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskMergeConflictRead {
    pub conflict_set: Option<TaskConflictSet>,
    pub dependencies_pending: bool,
}

impl std::fmt::Display for TaskMergeStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(error) | Self::Storage(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for TaskMergeStoreError {}

pub fn read_journal_page(
    connection: &Connection,
    after_position: i64,
    through_position: i64,
    limit: usize,
) -> Result<TaskMergeJournalPage, TaskMergeStoreError> {
    if after_position < 0 || through_position < after_position {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_invalid_cursor".into(),
        ));
    }
    if limit == 0 || limit > MAX_TASK_MERGE_PAGE_SIZE {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_page_limit_exceeded".into(),
        ));
    }

    let watermark: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(position), 0) FROM task_merge_journal",
            [],
            |row| row.get(0),
        )
        .map_err(storage_error)?;
    if through_position > watermark {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_watermark_from_future".into(),
        ));
    }

    let mut statement = connection
        .prepare(
            "SELECT position, record_kind, record_id FROM task_merge_journal
             WHERE position > ?1 AND position <= ?2
             ORDER BY position LIMIT ?3",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map(
            rusqlite::params![after_position, through_position, limit as i64],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(storage_error)?;
    let entries = rows
        .map(|row| {
            let (position, record_kind, record_id) = row.map_err(storage_error)?;
            let record = match record_kind.as_str() {
                "operation" => read_operation(connection, &record_id)?,
                "bootstrap" => read_bootstrap(connection, &record_id)?,
                _ => {
                    return Err(TaskMergeStoreError::Invalid(
                        "task_merge_journal_record_kind_invalid".into(),
                    ));
                }
            };
            Ok(TaskMergeJournalEntry { position, record })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TaskMergeJournalPage {
        through_position,
        entries,
    })
}

pub fn read_conflict_set(
    connection: &Connection,
    object_id: &str,
    unit: TaskMergeUnit,
) -> Result<TaskMergeConflictRead, TaskMergeStoreError> {
    let unit_key = serde_json::to_string(&unit)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    let mut statement = connection
        .prepare(
            "SELECT operation_id, operation_json FROM task_merge_operations
             WHERE object_id = ?1 AND unit = ?2 ORDER BY operation_id",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map(rusqlite::params![object_id, unit_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage_error)?;
    let operations = rows
        .map(|row| {
            let (stored_id, json) = row.map_err(storage_error)?;
            let operation = serde_json::from_str::<TaskMergeOperation>(&json)
                .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
            if operation.id.as_str() != stored_id
                || operation.object_id != object_id
                || operation.unit != unit
            {
                return Err(TaskMergeStoreError::Invalid(
                    "task_merge_operation_identity_mismatch".into(),
                ));
            }
            Ok(operation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (conflict_set, history_pending) = conflict_set_for_stored(operations)?;
    let dependencies_pending =
        history_pending || task_causal_dependencies_pending(connection, object_id)?;
    Ok(TaskMergeConflictRead {
        conflict_set,
        dependencies_pending,
    })
}

pub fn task_causal_dependencies_pending(
    connection: &Connection,
    object_id: &str,
) -> Result<bool, TaskMergeStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT operation_id, operation_json FROM task_merge_operations
             WHERE object_id = ?1 ORDER BY operation_id",
        )
        .map_err(storage_error)?;
    let stored = statement
        .query_map([object_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    drop(statement);

    let mut operations = BTreeMap::new();
    for (stored_id, json) in stored {
        let operation = serde_json::from_str::<TaskMergeOperation>(&json)
            .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
        if operation.id.as_str() != stored_id || operation.object_id != object_id {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_operation_identity_mismatch".into(),
            ));
        }
        operations.insert(operation.id.clone(), operation);
    }

    let mut pending = false;
    for operation in operations.values() {
        for reference in operation.parents.iter().chain(&operation.observed_heads) {
            if operations.contains_key(reference) {
                continue;
            }
            let referenced_object = connection
                .query_row(
                    "SELECT object_id FROM task_merge_operations WHERE operation_id = ?1",
                    [reference.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(storage_error)?;
            match referenced_object {
                Some(referenced_object) if referenced_object != object_id => {
                    return Err(TaskMergeStoreError::Invalid(
                        "task_merge_cross_object_dependency".into(),
                    ));
                }
                Some(_) => {
                    return Err(TaskMergeStoreError::Invalid(
                        "task_merge_dependency_not_in_task_history".into(),
                    ));
                }
                None => pending = true,
            }
        }
    }
    if !pending && !operations.is_empty() {
        ark_domain::task_merge::lifecycle_conflicts(
            &operations.values().cloned().collect::<Vec<_>>(),
        )
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    }
    Ok(pending)
}

fn conflict_set_for_stored(
    operations: Vec<TaskMergeOperation>,
) -> Result<(Option<TaskConflictSet>, bool), TaskMergeStoreError> {
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
    let set =
        conflict_set(&applied).map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    Ok((Some(set), !pending.is_empty()))
}

pub fn canonical_operation(
    mut operation: TaskMergeOperation,
) -> Result<(TaskMergeOperation, String, String), TaskMergeStoreError> {
    ark_domain::task_merge::validate_operation(&operation)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    operation.parents.sort();
    operation.observed_heads.sort();
    let value = canonical_value(
        &serde_json::to_value(&operation)
            .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?,
    );
    let json = serde_json::to_string(&value)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    let hash = format!("{:x}", Sha256::digest(json.as_bytes()));
    Ok((operation, json, hash))
}

pub fn canonical_bootstrap(
    bootstrap: TaskMergeBootstrap,
) -> Result<(TaskMergeBootstrap, String, String), TaskMergeStoreError> {
    bootstrap
        .baseline_operations()
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    let value = canonical_value(
        &serde_json::to_value(&bootstrap)
            .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?,
    );
    let json = serde_json::to_string(&value)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    let digest = format!("{:x}", Sha256::digest(json.as_bytes()));
    Ok((bootstrap, json, digest))
}

pub fn verify_bootstrap_payload(
    bootstrap: TaskMergeBootstrap,
    expected_json: &str,
    expected_digest: &str,
) -> Result<TaskMergeBootstrap, TaskMergeStoreError> {
    let (bootstrap, json, digest) = canonical_bootstrap(bootstrap)?;
    if json != expected_json || digest != expected_digest {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_bootstrap_payload_mismatch".into(),
        ));
    }
    Ok(bootstrap)
}

fn read_operation(
    connection: &Connection,
    stored_id: &str,
) -> Result<TaskMergeJournalRecord, TaskMergeStoreError> {
    let (json, stored_hash, expected_conflict_digest) = connection
        .query_row(
            "SELECT operation.operation_json, operation.payload_sha256,
                    guard.expected_conflict_digest
             FROM task_merge_operations AS operation
             LEFT JOIN task_merge_resolution_guards AS guard
                 ON guard.operation_id = operation.operation_id
             WHERE operation.operation_id = ?1",
            [stored_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .map_err(storage_error)?;
    let operation = serde_json::from_str::<TaskMergeOperation>(&json)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    let (operation, canonical_json, payload_hash) = canonical_operation(operation)?;
    if operation.id.as_str() != stored_id || canonical_json != json || payload_hash != stored_hash {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_journal_payload_mismatch".into(),
        ));
    }
    Ok(TaskMergeJournalRecord::Operation {
        operation,
        expected_conflict_digest,
    })
}

fn read_bootstrap(
    connection: &Connection,
    stored_id: &str,
) -> Result<TaskMergeJournalRecord, TaskMergeStoreError> {
    let (json, stored_hash) = connection
        .query_row(
            "SELECT bootstrap_json, payload_sha256 FROM task_merge_bootstraps
             WHERE bootstrap_id = ?1",
            [stored_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(storage_error)?;
    let bootstrap = serde_json::from_str(&json)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    let bootstrap = verify_bootstrap_payload(bootstrap, &json, &stored_hash)?;
    if bootstrap.id.as_str() != stored_id {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_journal_payload_mismatch".into(),
        ));
    }
    Ok(TaskMergeJournalRecord::Bootstrap(bootstrap))
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

fn storage_error(error: impl std::fmt::Display) -> TaskMergeStoreError {
    TaskMergeStoreError::Storage(error.to_string())
}

#[cfg(test)]
#[path = "task_merge/tests.rs"]
mod tests;
