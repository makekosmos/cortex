use super::{canonical_bootstrap, canonical_operation, storage_error, TaskMergeStoreError};
use ark_domain::task_merge::{TaskMergeBootstrap, TaskMergeOperation};
use rusqlite::{Connection, OptionalExtension};
use std::collections::BTreeSet;

#[derive(Debug)]
pub enum TaskMergeBootstrapCheck {
    Replay,
    New(Box<PreparedTaskMergeBootstrap>),
}

#[derive(Debug)]
pub struct PreparedTaskMergeBootstrap {
    bootstrap: TaskMergeBootstrap,
    bootstrap_json: String,
    payload_sha256: String,
}

impl PreparedTaskMergeBootstrap {
    pub fn bootstrap(&self) -> &TaskMergeBootstrap {
        &self.bootstrap
    }
}

pub fn check_bootstrap(
    connection: &Connection,
    bootstrap: &TaskMergeBootstrap,
) -> Result<TaskMergeBootstrapCheck, TaskMergeStoreError> {
    let (bootstrap, bootstrap_json, payload_sha256) = canonical_bootstrap(bootstrap.clone())?;
    let stored_hash = connection
        .query_row(
            "SELECT payload_sha256 FROM task_merge_bootstraps WHERE bootstrap_id = ?1",
            [bootstrap.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?;
    if let Some(stored_hash) = stored_hash {
        if stored_hash != payload_sha256 {
            return Err(TaskMergeStoreError::OperationIdCollision);
        }
        return Ok(TaskMergeBootstrapCheck::Replay);
    }

    let existing_bootstrap = connection
        .query_row(
            "SELECT bootstrap_id FROM task_merge_bootstraps WHERE object_id = ?1",
            [&bootstrap.object.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?;
    if existing_bootstrap.is_some() {
        return Err(TaskMergeStoreError::OperationIdCollision);
    }

    bootstrap
        .baseline_operations()
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    Ok(TaskMergeBootstrapCheck::New(Box::new(
        PreparedTaskMergeBootstrap {
            bootstrap,
            bootstrap_json,
            payload_sha256,
        },
    )))
}

pub fn persist_bootstrap(
    transaction: &Connection,
    prepared: Box<PreparedTaskMergeBootstrap>,
) -> Result<(), TaskMergeStoreError> {
    let PreparedTaskMergeBootstrap {
        bootstrap,
        bootstrap_json,
        payload_sha256,
    } = *prepared;
    let baseline = bootstrap
        .baseline_operations()
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    transaction
        .execute(
            "INSERT INTO task_merge_bootstraps(
                bootstrap_id, object_id, bootstrap_json, payload_sha256
             ) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                bootstrap.id.as_str(),
                bootstrap.object.id,
                bootstrap_json,
                payload_sha256,
            ],
        )
        .map_err(storage_error)?;

    for operation in baseline {
        insert_baseline_operation(transaction, operation)?;
    }

    transaction
        .execute(
            "INSERT INTO task_merge_journal(record_kind, record_id)
             VALUES ('bootstrap', ?1)",
            [bootstrap.id.as_str()],
        )
        .map_err(storage_error)?;
    journal_prebootstrap_operations(transaction, &bootstrap)?;
    Ok(())
}

pub fn load_bootstrap(
    connection: &Connection,
    object_id: &str,
) -> Result<TaskMergeBootstrap, TaskMergeStoreError> {
    let (json, digest) = connection
        .query_row(
            "SELECT bootstrap_json, payload_sha256 FROM task_merge_bootstraps
             WHERE object_id = ?1",
            [object_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(storage_error)?;
    let bootstrap = serde_json::from_str(&json)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    super::verify_bootstrap_payload(bootstrap, &json, &digest)
}

pub fn has_bootstrap(
    connection: &Connection,
    object_id: &str,
) -> Result<bool, TaskMergeStoreError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM task_merge_bootstraps WHERE object_id = ?1)",
            [object_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage_error)
}

fn journal_prebootstrap_operations(
    transaction: &Connection,
    bootstrap: &TaskMergeBootstrap,
) -> Result<(), TaskMergeStoreError> {
    let baseline_ids = bootstrap
        .baseline_ids
        .iter()
        .map(|(_, id)| id.as_str())
        .collect::<BTreeSet<_>>();
    let mut statement = transaction
        .prepare(
            "SELECT operation_id FROM task_merge_operations
             WHERE object_id = ?1 ORDER BY rowid",
        )
        .map_err(storage_error)?;
    let operation_ids = statement
        .query_map([&bootstrap.object.id], |row| row.get::<_, String>(0))
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    drop(statement);

    for operation_id in operation_ids {
        if baseline_ids.contains(operation_id.as_str()) {
            continue;
        }
        let already_journaled = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_merge_journal
                 WHERE record_kind = 'operation' AND record_id = ?1)",
                [&operation_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(storage_error)?;
        if !already_journaled {
            transaction
                .execute(
                    "INSERT INTO task_merge_journal(record_kind, record_id)
                     VALUES ('operation', ?1)",
                    [&operation_id],
                )
                .map_err(storage_error)?;
        }
    }
    Ok(())
}

fn insert_baseline_operation(
    transaction: &Connection,
    operation: TaskMergeOperation,
) -> Result<(), TaskMergeStoreError> {
    let (operation, operation_json, payload_sha256) = canonical_operation(operation)?;
    let stored_hash = transaction
        .query_row(
            "SELECT payload_sha256 FROM task_merge_operations WHERE operation_id = ?1",
            [operation.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?;
    if let Some(stored_hash) = stored_hash {
        return if stored_hash == payload_sha256 {
            Ok(())
        } else {
            Err(TaskMergeStoreError::OperationIdCollision)
        };
    }

    let unit = serde_json::to_string(&operation.unit)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
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
    Ok(())
}
