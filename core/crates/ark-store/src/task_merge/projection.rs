use super::{has_bootstrap, load_bootstrap, read_conflict_set, TaskMergeStoreError};
use ark_domain::task_merge::{materialize_task, TaskMergeUnit, TaskMergeValue};
use ark_domain::types::ArkObject;
use rusqlite::Connection;

#[derive(Debug, Clone, PartialEq)]
pub enum TaskMergeProjectionOutcome {
    NoBootstrap,
    DependenciesPending,
    Materialized {
        object: Option<Box<ArkObject>>,
        hard_deleted: bool,
    },
}

pub fn materialize_task_if_ready(
    connection: &Connection,
    object_id: &str,
) -> Result<TaskMergeProjectionOutcome, TaskMergeStoreError> {
    if !has_bootstrap(connection, object_id)? {
        return Ok(TaskMergeProjectionOutcome::NoBootstrap);
    }
    let current_object =
        crate::objects::get_object(connection, object_id).map_err(super::storage_error)?;
    let projection_missing = current_object.is_none();
    let bootstrap = if projection_missing {
        Some(load_bootstrap(connection, object_id)?)
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
        let read = read_conflict_set(connection, object_id, unit)?;
        if read.dependencies_pending {
            return Ok(TaskMergeProjectionOutcome::DependenciesPending);
        }
        if let Some(conflict_set) = read.conflict_set {
            conflict_sets.push(conflict_set);
        }
    }

    let materialized = materialize_task(&object, &conflict_sets)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
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
        matches!(operation.value, TaskMergeValue::ObjectHardDeleted { .. })
    });
    let deleted = existence.is_some_and(|operation| {
        matches!(
            operation.value,
            TaskMergeValue::ObjectDeleted { .. } | TaskMergeValue::ObjectHardDeleted { .. }
        )
    });

    let object = if hard_deleted {
        crate::objects::delete_object(connection, object_id).map_err(super::storage_error)?;
        connection
            .execute(
                "DELETE FROM object_links WHERE source_object_id = ?1",
                [object_id],
            )
            .map_err(super::storage_error)?;
        None
    } else {
        if materialized != object || projection_missing {
            crate::objects::upsert_object(connection, &materialized)
                .map_err(super::storage_error)?;
        }
        if projection_missing {
            for link in &baseline_links {
                crate::objects::upsert_object_link(connection, link)
                    .map_err(super::storage_error)?;
            }
        }
        Some(materialized)
    };

    if let Some(revision) = revision {
        connection
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc,
                     deleted = excluded.deleted
                 WHERE excluded.hlc >= object_sync_versions.hlc",
                rusqlite::params![object_id, revision, deleted],
            )
            .map_err(super::storage_error)?;
        if deleted {
            record_task_tombstone(connection, object_id, revision)?;
        } else {
            connection
                .execute("DELETE FROM sync_tombstones WHERE id = ?1", [object_id])
                .map_err(super::storage_error)?;
        }
    }

    Ok(TaskMergeProjectionOutcome::Materialized {
        object: object.map(Box::new),
        hard_deleted,
    })
}

fn record_task_tombstone(
    connection: &Connection,
    object_id: &str,
    revision: &str,
) -> Result<(), TaskMergeStoreError> {
    connection
        .execute(
            "INSERT INTO sync_tombstones (id, entity_type, hlc, deleted_at, type_id)
             VALUES (?1, 'object', ?2, ?2, 'core.task')
             ON CONFLICT(id) DO UPDATE SET
                entity_type = excluded.entity_type,
                hlc = excluded.hlc,
                deleted_at = excluded.deleted_at,
                type_id = COALESCE(excluded.type_id, sync_tombstones.type_id)
             WHERE excluded.hlc >= sync_tombstones.hlc",
            rusqlite::params![object_id, revision],
        )
        .map_err(super::storage_error)?;
    Ok(())
}
