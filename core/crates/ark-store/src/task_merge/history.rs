use super::{storage_error, TaskMergeConflictRead, TaskMergeStoreError};
use ark_domain::task_merge::{conflict_set, TaskConflictSet, TaskMergeOperation, TaskMergeUnit};
use rusqlite::{Connection, OptionalExtension};
use std::collections::{BTreeMap, BTreeSet};

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
