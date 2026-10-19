use super::{read_conflict_set, TaskMergeError};
use ark_domain::task_merge::{materialize_task, TaskMergeBootstrap, TaskMergeUnit};
use rusqlite::Connection;

pub(super) fn append_bootstrap(
    transaction: &Connection,
    bootstrap: TaskMergeBootstrap,
) -> Result<bool, TaskMergeError> {
    append_bootstrap_with_revision(transaction, bootstrap, true)
}

pub(super) fn append_local_bootstrap(
    transaction: &Connection,
    bootstrap: TaskMergeBootstrap,
) -> Result<bool, TaskMergeError> {
    append_bootstrap_with_revision(transaction, bootstrap, false)
}

fn append_bootstrap_with_revision(
    transaction: &Connection,
    bootstrap: TaskMergeBootstrap,
    record_revision: bool,
) -> Result<bool, TaskMergeError> {
    let object_id = bootstrap.object.id.clone();
    let prepared = match ark_store::task_merge::check_bootstrap(transaction, &bootstrap)? {
        ark_store::task_merge::TaskMergeBootstrapCheck::Replay => {
            materialize_if_ready(transaction, &object_id)?;
            return Ok(true);
        }
        ark_store::task_merge::TaskMergeBootstrapCheck::New(prepared) => prepared,
    };
    let bootstrap = prepared.bootstrap().clone();

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

    ark_store::task_merge::persist_bootstrap(transaction, prepared)?;
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

    if record_revision {
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
    }

    materialize_if_ready(transaction, &bootstrap.object.id)?;
    Ok(false)
}

pub(super) fn materialize_if_ready(
    transaction: &Connection,
    object_id: &str,
) -> Result<bool, TaskMergeError> {
    let has_bootstrap = ark_store::task_merge::has_bootstrap(transaction, object_id)
        .map_err(TaskMergeError::from)?;
    if !has_bootstrap {
        return Ok(false);
    }
    let current_object = crate::db::get_object(transaction, object_id).map_err(storage_error)?;
    let projection_missing = current_object.is_none();
    let bootstrap = if projection_missing {
        Some(ark_store::task_merge::load_bootstrap(
            transaction,
            object_id,
        )?)
    } else {
        None
    };
    let object = current_object.unwrap_or_else(|| {
        bootstrap
            .as_ref()
            .expect("missing projection requires bootstrap")
            .object
            .clone()
    });
    let baseline_links = bootstrap
        .map(|bootstrap| bootstrap.links)
        .unwrap_or_default();

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
    let existence = conflict_sets
        .iter()
        .find(|set| set.unit == TaskMergeUnit::ObjectExistence)
        .and_then(|set| {
            set.variants
                .iter()
                .min_by(|left, right| left.id.cmp(&right.id))
        });
    let revision = existence.and_then(|operation| operation.hlc.as_deref());
    let hard_deleted = existence.is_some_and(|operation| {
        matches!(
            operation.value,
            ark_domain::task_merge::TaskMergeValue::ObjectHardDeleted { .. }
        )
    });
    let deleted = existence.is_some_and(|operation| {
        matches!(
            operation.value,
            ark_domain::task_merge::TaskMergeValue::ObjectDeleted { .. }
                | ark_domain::task_merge::TaskMergeValue::ObjectHardDeleted { .. }
        )
    });
    if hard_deleted {
        crate::db::delete_object(transaction, object_id).map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM object_links WHERE source_object_id = ?1",
                [object_id],
            )
            .map_err(storage_error)?;
    } else {
        if materialized != object || projection_missing {
            crate::db::upsert_object(transaction, &materialized).map_err(storage_error)?;
        }
        if projection_missing {
            for link in &baseline_links {
                crate::db::upsert_object_link(transaction, link).map_err(storage_error)?;
            }
        }
    }
    if let Some(revision) = revision {
        transaction
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc,
                     deleted = excluded.deleted
                 WHERE excluded.hlc >= object_sync_versions.hlc",
                rusqlite::params![object_id, revision, deleted],
            )
            .map_err(storage_error)?;
        if deleted {
            crate::db::record_sync_tombstone_with_type(
                transaction,
                "object",
                object_id,
                Some("core.task"),
                revision,
            )
            .map_err(storage_error)?;
        } else {
            crate::db::delete_sync_tombstone(transaction, object_id).map_err(storage_error)?;
        }
    }
    Ok(true)
}

fn storage_error(error: impl std::fmt::Display) -> TaskMergeError {
    TaskMergeError::Storage(error.to_string())
}
