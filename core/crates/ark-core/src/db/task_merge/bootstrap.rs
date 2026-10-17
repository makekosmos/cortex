use super::{canonical_operation, read_conflict_set, TaskMergeError};
use crate::domain::task_merge::{materialize_task, TaskMergeBootstrap, TaskMergeUnit};
use rusqlite::{OptionalExtension, Transaction};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) fn append_bootstrap(
    transaction: &Transaction<'_>,
    bootstrap: TaskMergeBootstrap,
) -> Result<bool, TaskMergeError> {
    let (bootstrap, bootstrap_json, payload_sha256) = canonical_bootstrap(bootstrap)?;
    if let Some(stored_hash) = transaction
        .query_row(
            "SELECT payload_sha256 FROM task_merge_bootstraps WHERE bootstrap_id = ?1",
            [bootstrap.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?
    {
        if stored_hash != payload_sha256 {
            return Err(TaskMergeError::OperationIdCollision);
        }
        materialize_if_ready(transaction, &bootstrap.object.id)?;
        return Ok(true);
    }

    let existing_bootstrap = transaction
        .query_row(
            "SELECT bootstrap_id FROM task_merge_bootstraps WHERE object_id = ?1",
            [&bootstrap.object.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?;
    if existing_bootstrap.is_some() {
        return Err(TaskMergeError::OperationIdCollision);
    }

    let existing_object =
        crate::db::get_object(transaction, &bootstrap.object.id).map_err(storage_error)?;
    if let Some(existing_object) = existing_object {
        let mut existing_links = crate::db::list_object_links(transaction)
            .map_err(storage_error)?
            .into_iter()
            .filter(|link| link.source_object_id == bootstrap.object.id)
            .collect::<Vec<_>>();
        existing_links.sort_by(|left, right| left.id.cmp(&right.id));
        let mut incoming_links = bootstrap.links.clone();
        incoming_links.sort_by(|left, right| left.id.cmp(&right.id));
        if existing_object != bootstrap.object || existing_links != incoming_links {
            return Err(TaskMergeError::Invalid(
                "task_merge_bootstrap_existing_object_mismatch".into(),
            ));
        }
    }

    let baseline = bootstrap
        .baseline_operations()
        .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
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

    crate::db::upsert_object(transaction, &bootstrap.object).map_err(storage_error)?;
    transaction
        .execute(
            "DELETE FROM object_links WHERE source_object_id = ?1",
            [&bootstrap.object.id],
        )
        .map_err(storage_error)?;
    for link in &bootstrap.links {
        crate::db::upsert_object_link(transaction, link).map_err(storage_error)?;
    }

    crate::db::record_remote_sync_revision(
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
        crate::db::delete_sync_tombstone(transaction, &bootstrap.object.id)
            .map_err(storage_error)?;
    }

    materialize_if_ready(transaction, &bootstrap.object.id)?;
    Ok(false)
}

fn journal_prebootstrap_operations(
    transaction: &Transaction<'_>,
    bootstrap: &TaskMergeBootstrap,
) -> Result<(), TaskMergeError> {
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

pub(super) fn materialize_if_ready(
    transaction: &Transaction<'_>,
    object_id: &str,
) -> Result<bool, TaskMergeError> {
    let has_bootstrap = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM task_merge_bootstraps WHERE object_id = ?1)",
            [object_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage_error)?;
    if !has_bootstrap {
        return Ok(false);
    }
    let Some(mut object) = crate::db::get_object(transaction, object_id).map_err(storage_error)?
    else {
        return Err(TaskMergeError::Storage(
            "task_merge_bootstrap_object_missing".into(),
        ));
    };

    let mut conflict_sets = Vec::with_capacity(TaskMergeUnit::ALL.len());
    for unit in TaskMergeUnit::ALL {
        let (conflict_set, dependencies_pending) = read_conflict_set(transaction, object_id, unit)?;
        if dependencies_pending {
            return Ok(false);
        }
        if let Some(conflict_set) = conflict_set {
            conflict_sets.push(conflict_set);
        }
    }

    let materialized = materialize_task(&object, &conflict_sets)
        .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    if materialized != object {
        object = materialized;
        crate::db::upsert_object(transaction, &object).map_err(storage_error)?;
    }
    Ok(true)
}

fn insert_baseline_operation(
    transaction: &Transaction<'_>,
    operation: crate::domain::task_merge::TaskMergeOperation,
) -> Result<(), TaskMergeError> {
    let (operation, operation_json, payload_sha256) = canonical_operation(operation)?;
    if let Some(stored_hash) = transaction
        .query_row(
            "SELECT payload_sha256 FROM task_merge_operations WHERE operation_id = ?1",
            [operation.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?
    {
        return if stored_hash == payload_sha256 {
            Ok(())
        } else {
            Err(TaskMergeError::OperationIdCollision)
        };
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
        .map_err(storage_error)?;
    Ok(())
}

pub(super) fn canonical_bootstrap(
    bootstrap: TaskMergeBootstrap,
) -> Result<(TaskMergeBootstrap, String, String), TaskMergeError> {
    bootstrap
        .baseline_operations()
        .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    let value = super::canonical_value(
        &serde_json::to_value(&bootstrap)
            .map_err(|error| TaskMergeError::Invalid(error.to_string()))?,
    );
    let json = serde_json::to_string(&value)
        .map_err(|error| TaskMergeError::Invalid(error.to_string()))?;
    let digest = format!("{:x}", Sha256::digest(json.as_bytes()));
    Ok((bootstrap, json, digest))
}

pub(super) fn verify_bootstrap_payload(
    bootstrap: TaskMergeBootstrap,
    expected_json: &str,
    expected_digest: &str,
) -> Result<TaskMergeBootstrap, TaskMergeError> {
    let (bootstrap, json, digest) = canonical_bootstrap(bootstrap)?;
    if json != expected_json || digest != expected_digest {
        return Err(TaskMergeError::Invalid(
            "task_merge_bootstrap_payload_mismatch".into(),
        ));
    }
    Ok(bootstrap)
}

fn storage_error(error: impl std::fmt::Display) -> TaskMergeError {
    TaskMergeError::Storage(error.to_string())
}
