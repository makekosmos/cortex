use super::{
    canonical_operation, read_conflict_set, storage_error, TaskMergeConflictRead,
    TaskMergeStoreError,
};
use ark_domain::task_merge::TaskMergeOperation;
use rusqlite::{Connection, OptionalExtension};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskMergeOperationOrigin {
    Local,
    Remote,
}

#[derive(Debug)]
pub enum TaskMergeOperationCheck {
    Replay(TaskMergeConflictRead),
    New(PreparedTaskMergeOperation),
}

#[derive(Debug)]
pub struct PreparedTaskMergeOperation {
    operation: TaskMergeOperation,
    operation_json: String,
    payload_sha256: String,
    unit: String,
    expected_conflict_digest: Option<String>,
    origin: TaskMergeOperationOrigin,
}

impl PreparedTaskMergeOperation {
    pub fn operation(&self) -> &TaskMergeOperation {
        &self.operation
    }
}

pub fn check_operation(
    connection: &Connection,
    operation: &TaskMergeOperation,
    expected_conflict_digest: Option<&str>,
    origin: TaskMergeOperationOrigin,
) -> Result<TaskMergeOperationCheck, TaskMergeStoreError> {
    let (operation, operation_json, payload_sha256) = canonical_operation(operation.clone())?;
    let stored_hash = connection
        .query_row(
            "SELECT payload_sha256 FROM task_merge_operations WHERE operation_id = ?1",
            [operation.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?;
    if let Some(stored_hash) = stored_hash {
        if stored_hash != payload_sha256 {
            return Err(TaskMergeStoreError::OperationIdCollision);
        }
        if origin == TaskMergeOperationOrigin::Remote {
            let stored_guard = connection
                .query_row(
                    "SELECT expected_conflict_digest
                     FROM task_merge_resolution_guards WHERE operation_id = ?1",
                    [operation.id.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(storage_error)?;
            if stored_guard.as_deref() != expected_conflict_digest {
                return Err(TaskMergeStoreError::OperationIdCollision);
            }
        }
        return read_conflict_set(connection, &operation.object_id, operation.unit)
            .map(TaskMergeOperationCheck::Replay);
    }

    let unit = serde_json::to_string(&operation.unit)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    Ok(TaskMergeOperationCheck::New(PreparedTaskMergeOperation {
        operation,
        operation_json,
        payload_sha256,
        unit,
        expected_conflict_digest: expected_conflict_digest.map(str::to_owned),
        origin,
    }))
}

pub fn persist_operation(
    transaction: &Connection,
    prepared: PreparedTaskMergeOperation,
) -> Result<(), TaskMergeStoreError> {
    let PreparedTaskMergeOperation {
        operation,
        operation_json,
        payload_sha256,
        unit,
        expected_conflict_digest,
        origin,
    } = prepared;
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
        .map_err(storage_error)?;
    if let Some(expected_digest) = expected_conflict_digest {
        transaction
            .execute(
                "INSERT INTO task_merge_resolution_guards(
                    operation_id, expected_conflict_digest
                 ) VALUES (?1, ?2)",
                rusqlite::params![operation.id.as_str(), expected_digest],
            )
            .map_err(storage_error)?;
    }
    let has_bootstrap = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM task_merge_bootstraps WHERE object_id = ?1)",
            [&operation.object_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage_error)?;
    if has_bootstrap || origin == TaskMergeOperationOrigin::Local {
        transaction
            .execute(
                "INSERT INTO task_merge_journal(record_kind, record_id)
                 VALUES ('operation', ?1)",
                [operation.id.as_str()],
            )
            .map_err(storage_error)?;
    }
    Ok(())
}
