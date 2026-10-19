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

pub fn apply_inbound_bootstrap(
    transaction: &Connection,
    bootstrap: &TaskMergeBootstrap,
) -> Result<bool, TaskMergeStoreError> {
    let object_id = bootstrap.object.id.clone();
    let prepared = match check_bootstrap(transaction, bootstrap)? {
        TaskMergeBootstrapCheck::Replay => {
            super::materialize_task_if_ready(transaction, &object_id)?;
            return Ok(true);
        }
        TaskMergeBootstrapCheck::New(prepared) => prepared,
    };
    let bootstrap = prepared.bootstrap().clone();

    if let Some(existing_object) =
        crate::objects::get_object(transaction, &bootstrap.object.id).map_err(storage_error)?
    {
        let mut existing_links = crate::objects::list_object_links(transaction)
            .map_err(storage_error)?
            .into_iter()
            .filter(|link| link.source_object_id == bootstrap.object.id)
            .collect::<Vec<_>>();
        existing_links.sort_by(|left, right| left.id.cmp(&right.id));
        let mut incoming_links = bootstrap.links.clone();
        incoming_links.sort_by(|left, right| left.id.cmp(&right.id));
        if existing_object != bootstrap.object || existing_links != incoming_links {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_bootstrap_existing_object_mismatch".into(),
            ));
        }
    }

    persist_bootstrap(transaction, prepared)?;
    crate::objects::upsert_object(transaction, &bootstrap.object).map_err(storage_error)?;
    transaction
        .execute(
            "DELETE FROM object_links WHERE source_object_id = ?1",
            [&bootstrap.object.id],
        )
        .map_err(storage_error)?;
    for link in &bootstrap.links {
        crate::objects::upsert_object_link(transaction, link).map_err(storage_error)?;
    }

    crate::sync_versions::record_remote_object_revision(
        transaction,
        &bootstrap.object.id,
        &bootstrap.source_hlc,
    )
    .map_err(storage_error)?;
    transaction
        .execute(
            "INSERT INTO object_sync_versions(object_id, hlc, deleted)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc,
                 deleted = excluded.deleted
             WHERE excluded.hlc > object_sync_versions.hlc",
            rusqlite::params![
                bootstrap.object.id,
                bootstrap.source_hlc,
                bootstrap.object.deleted_at.is_some(),
            ],
        )
        .map_err(storage_error)?;
    if bootstrap.object.deleted_at.is_none() {
        transaction
            .execute(
                "DELETE FROM sync_tombstones WHERE id = ?1",
                [&bootstrap.object.id],
            )
            .map_err(storage_error)?;
    }

    super::materialize_task_if_ready(transaction, &bootstrap.object.id)?;
    Ok(false)
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
