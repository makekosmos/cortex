use super::{has_bootstrap, read_conflict_set, storage_error, TaskMergeStoreError};
use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::task_merge::{TaskConflictSet, TaskMergeUnit};
use ark_domain::types::{ArkObject, ObjectLink};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub struct TaskMergeLocalMutationState {
    pub object: Option<ArkObject>,
    pub source_links: Vec<ObjectLink>,
    pub has_bootstrap: bool,
    pub snapshot: ObjectWriteSnapshot,
    pub legacy_revision: Option<String>,
    pub unit_conflicts: Vec<(TaskMergeUnit, TaskConflictSet)>,
}

pub fn load_local_mutation_state(
    transaction: &Connection,
    object_id: &str,
) -> Result<TaskMergeLocalMutationState, TaskMergeStoreError> {
    let object = crate::objects::get_object(transaction, object_id).map_err(storage_error)?;
    let mut source_links = crate::objects::list_object_links(transaction)
        .map_err(storage_error)?
        .into_iter()
        .filter(|link| link.source_object_id == object_id)
        .collect::<Vec<_>>();
    source_links.sort_by(|left, right| left.id.cmp(&right.id));

    let has_bootstrap = has_bootstrap(transaction, object_id)?;
    let unit_conflicts = if has_bootstrap {
        load_unit_conflicts(transaction, object_id)?
    } else {
        Vec::new()
    };
    let task_revision = task_snapshot_token_from_conflicts(has_bootstrap, &unit_conflicts)?;
    let legacy_revision =
        crate::sync_versions::object_revision(transaction, object_id).map_err(storage_error)?;
    let snapshot = ObjectWriteSnapshot {
        exists: object.is_some(),
        type_id: object.as_ref().map(|value| value.type_id.clone()),
        type_version: object.as_ref().map(|value| value.type_version.clone()),
        revision: task_revision.or_else(|| legacy_revision.clone()),
    };

    Ok(TaskMergeLocalMutationState {
        object,
        source_links,
        has_bootstrap,
        snapshot,
        legacy_revision,
        unit_conflicts,
    })
}

pub fn task_snapshot_token(
    transaction: &Connection,
    object_id: &str,
) -> Result<Option<String>, TaskMergeStoreError> {
    if !has_bootstrap(transaction, object_id)? {
        return Ok(None);
    }
    let unit_conflicts = load_unit_conflicts(transaction, object_id)?;
    task_snapshot_token_from_conflicts(true, &unit_conflicts)
}

fn load_unit_conflicts(
    transaction: &Connection,
    object_id: &str,
) -> Result<Vec<(TaskMergeUnit, TaskConflictSet)>, TaskMergeStoreError> {
    let mut conflicts = Vec::with_capacity(TaskMergeUnit::ALL.len());
    for unit in TaskMergeUnit::ALL {
        let read = read_conflict_set(transaction, object_id, unit)?;
        if read.dependencies_pending {
            return Err(TaskMergeStoreError::Invalid(
                "task_merge_dependencies_pending".into(),
            ));
        }
        let set = read.conflict_set.ok_or_else(|| {
            TaskMergeStoreError::Invalid("task_merge_bootstrap_incomplete".into())
        })?;
        conflicts.push((unit, set));
    }
    Ok(conflicts)
}

fn task_snapshot_token_from_conflicts(
    has_bootstrap: bool,
    unit_conflicts: &[(TaskMergeUnit, TaskConflictSet)],
) -> Result<Option<String>, TaskMergeStoreError> {
    if !has_bootstrap {
        return Ok(None);
    }
    let heads = unit_conflicts
        .iter()
        .map(|(unit, set)| (*unit, set.digest.clone()))
        .collect::<Vec<_>>();
    if heads.len() != TaskMergeUnit::ALL.len() {
        return Err(TaskMergeStoreError::Invalid(
            "task_merge_bootstrap_incomplete".into(),
        ));
    }
    let encoded = serde_json::to_vec(&heads)
        .map_err(|error| TaskMergeStoreError::Invalid(error.to_string()))?;
    let mut digest = Sha256::new();
    digest.update(b"ark-task-snapshot-v1\0");
    digest.update(encoded);
    Ok(Some(format!("task-merge:sha256:{:x}", digest.finalize())))
}
