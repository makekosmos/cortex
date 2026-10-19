use super::{append_local_bootstrap, append_operation_inner, read_conflict_set};
use crate::types::{ArkObject, ObjectLink, SyncEntity};
use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::task_merge::{
    value_for_unit, OperationId, TaskMergeBootstrap, TaskMergeOperation, TaskMergeUnit,
};
use rusqlite::Connection;
use std::collections::BTreeSet;

/// Writes a local task through the causal journal. The caller already checked
/// the request's expected object revision in the same actor transaction.
pub(crate) fn commit_local_task_upsert(
    connection: &Connection,
    object: &ArkObject,
    links: &[ObjectLink],
    device_id: &str,
    expected_snapshot: Option<&ObjectWriteSnapshot>,
) -> Result<Vec<SyncEntity>, String> {
    commit_local_task_upsert_with_existence(
        connection,
        object,
        links,
        device_id,
        expected_snapshot,
        None,
    )
}

pub(crate) fn commit_local_task_delete(
    connection: &Connection,
    object_id: &str,
    device_id: &str,
    expected_snapshot: Option<&ObjectWriteSnapshot>,
    hard_delete: bool,
) -> Result<String, String> {
    let state = ark_store::task_merge::load_local_mutation_state(connection, object_id)
        .map_err(|error| error.to_string())?;
    let mut object = state.object.ok_or_else(|| "MISSING_OBJECT".to_string())?;
    let normalized_device_id = normalized_device_id(device_id);
    let deleted_at = crate::hlc::HLC::now(normalized_device_id).wall_time;
    object.deleted_at = Some(deleted_at.clone());
    let current_snapshot = if expected_snapshot.is_none() {
        Some(state.snapshot.clone())
    } else {
        None
    };
    let expected_snapshot = expected_snapshot.or(current_snapshot.as_ref());
    let links = state.source_links;
    let existence_value = Some(if hard_delete {
        ark_domain::task_merge::TaskMergeValue::ObjectHardDeleted { deleted_at }
    } else {
        ark_domain::task_merge::TaskMergeValue::ObjectDeleted { deleted_at }
    });
    let entities = commit_local_task_upsert_with_existence(
        connection,
        &object,
        &links,
        normalized_device_id,
        expected_snapshot,
        existence_value,
    )?;
    entities
        .into_iter()
        .next()
        .map(|entity| entity.hlc)
        .ok_or_else(|| "task_delete_missing_revision".to_string())
}

fn commit_local_task_upsert_with_existence(
    connection: &Connection,
    object: &ArkObject,
    links: &[ObjectLink],
    device_id: &str,
    expected_snapshot: Option<&ObjectWriteSnapshot>,
    existence_override: Option<ark_domain::task_merge::TaskMergeValue>,
) -> Result<Vec<SyncEntity>, String> {
    if object.type_id != "core.task" || object.type_version != "0.1.0" {
        return Err("object_write_slice_unsupported_type".into());
    }

    let state = ark_store::task_merge::load_local_mutation_state(connection, &object.id)
        .map_err(|error| error.to_string())?;
    let previous = state.object.clone();
    let has_bootstrap = state.has_bootstrap;
    if previous.is_some() || has_bootstrap {
        let Some(expected_snapshot) = expected_snapshot else {
            return Err("object_conflict:expected_snapshot_required".into());
        };
        if &state.snapshot != expected_snapshot {
            return Err("object_conflict:stale_snapshot".into());
        }
    }
    let previous_links = state.source_links;

    let local_revision = if previous.is_none() && !has_bootstrap {
        Some(record_local_revision(connection, &object.id, device_id)?)
    } else {
        None
    };
    let bootstrap_clock = match (&previous, &local_revision) {
        (_, Some(revision)) => revision.clone(),
        (Some(_), None) => state
            .legacy_revision
            .clone()
            .unwrap_or_else(|| crate::hlc::HLC::now(device_id).to_string()),
        (None, None) => crate::hlc::HLC::now(device_id).to_string(),
    };
    if !has_bootstrap {
        let baseline = previous.clone().unwrap_or_else(|| object.clone());
        let baseline_links = if previous.is_some() {
            previous_links.clone()
        } else {
            links.to_vec()
        };
        let bootstrap = TaskMergeBootstrap {
            id: next_operation_id(),
            object: baseline,
            links: baseline_links,
            source_hlc: bootstrap_clock,
            baseline_ids: TaskMergeUnit::ALL
                .into_iter()
                .map(|unit| (unit, next_operation_id()))
                .collect(),
        };
        append_local_bootstrap(connection, bootstrap).map_err(|error| error.to_string())?;
    }

    let revision = if let Some(revision) = local_revision {
        revision
    } else {
        record_local_revision(connection, &object.id, device_id)?
    };

    let mut visible_heads = BTreeSet::new();
    let mut unit_heads = Vec::with_capacity(TaskMergeUnit::ALL.len());
    for unit in TaskMergeUnit::ALL {
        let (set, dependencies_pending) =
            read_conflict_set(connection, &object.id, unit).map_err(|error| error.to_string())?;
        if dependencies_pending {
            return Err("task_merge_dependencies_pending".into());
        }
        let set = set.ok_or_else(|| "task_merge_bootstrap_incomplete".to_string())?;
        let winner = set
            .variants
            .iter()
            .min_by(|left, right| left.id.cmp(&right.id))
            .ok_or_else(|| "task_merge_empty_conflict_set".to_string())?;
        let visible_head = winner.id.clone();
        if unit == TaskMergeUnit::ObjectExistence && existence_override.is_some() {
            visible_heads.extend(set.variants.iter().map(|operation| operation.id.clone()));
        } else {
            visible_heads.insert(visible_head.clone());
        }
        unit_heads.push((unit, set, visible_head));
    }

    for (unit, set, visible_head) in unit_heads {
        let next_value = if unit == TaskMergeUnit::ObjectExistence {
            existence_override
                .clone()
                .unwrap_or(value_for_unit(object, unit).map_err(|error| error.to_string())?)
        } else {
            value_for_unit(object, unit).map_err(|error| error.to_string())?
        };
        let visible_value = set
            .variants
            .iter()
            .find(|operation| operation.id == visible_head)
            .ok_or_else(|| "task_merge_visible_head_missing".to_string())?;
        if visible_value.value == next_value {
            continue;
        }

        // Existing callers submit a full task projection. It only proves they
        // observed the deterministic projected head, so retain concurrent
        // alternatives instead of silently resolving them.
        let resolves_explicit_existence =
            unit == TaskMergeUnit::ObjectExistence && existence_override.is_some();
        let parents = if resolves_explicit_existence {
            set.variants
                .iter()
                .map(|operation| operation.id.clone())
                .collect()
        } else {
            vec![visible_head]
        };
        let operation = TaskMergeOperation {
            id: next_operation_id(),
            object_id: object.id.clone(),
            unit,
            parents,
            observed_heads: visible_heads.iter().cloned().collect(),
            hlc: Some(revision.clone()),
            value: next_value,
        };
        let operation_id = operation.id.clone();
        let expected_conflict_digest = resolves_explicit_existence.then_some(set.digest.as_str());
        append_operation_inner(
            connection,
            operation,
            expected_conflict_digest,
            ark_store::task_merge::TaskMergeOperationOrigin::Local,
        )
        .map_err(|error| error.to_string())?;
        visible_heads.insert(operation_id);
    }

    let materialized = crate::db::get_object(connection, &object.id)?;
    if let Some(materialized) = &materialized {
        validate_materialized_task(materialized)?;
    }

    let is_hard_deleted = existence_override.is_some_and(|value| {
        matches!(
            value,
            ark_domain::task_merge::TaskMergeValue::ObjectHardDeleted { .. }
        )
    });
    let entity = if let Some(materialized) = materialized {
        sync_entity("object", &materialized.id, &materialized, &revision)?
    } else if is_hard_deleted {
        SyncEntity {
            entity_type: "object".into(),
            id: object.id.clone(),
            data: serde_json::json!({"typeId": "core.task"})
                .as_object()
                .cloned()
                .unwrap_or_default(),
            hlc: revision.clone(),
            deleted: Some(true),
            origin_device_id: Some(crate::hlc::HLC::from_string(&revision).device_id),
            origin_seq: None,
        }
    } else {
        return Err("task_merge_materialized_object_missing".into());
    };

    let mut entities = vec![entity];
    if !is_hard_deleted {
        for link in links {
            crate::db::upsert_object_link(connection, link)?;
            let link_revision = crate::db::bump_sync_version_vector(
                connection,
                "object_link",
                &link.id,
                device_id,
                false,
            )?;
            entities.push(sync_entity("object_link", &link.id, link, &link_revision)?);
        }
    }
    Ok(entities)
}

fn record_local_revision(
    connection: &Connection,
    object_id: &str,
    device_id: &str,
) -> Result<String, String> {
    let device_id = normalized_device_id(device_id);
    let revision =
        crate::db::bump_sync_version_vector(connection, "object", object_id, device_id, false)?;
    connection
        .execute(
            "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 0)
             ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = 0
             WHERE excluded.hlc > object_sync_versions.hlc",
            rusqlite::params![object_id, revision],
        )
        .map_err(|error| error.to_string())?;
    crate::db::delete_sync_tombstone(connection, object_id)?;
    Ok(revision)
}

fn normalized_device_id(device_id: &str) -> &str {
    match device_id.trim() {
        "" => "ark-core-rpc-local",
        normalized => normalized,
    }
}

fn next_operation_id() -> OperationId {
    OperationId::parse(format!("{:032x}", rand::random::<u128>()))
        .expect("random u128 is a valid task merge operation ID")
}

fn validate_materialized_task(object: &ArkObject) -> Result<(), String> {
    let registration = crate::canonical_types::definitions::canonical_type_registrations()?
        .into_iter()
        .find(|registration| {
            registration.type_id == object.type_id && registration.version == object.type_version
        })
        .ok_or_else(|| "canonical_registry".to_string())?;
    crate::canonical_types::validation::validate_canonical(
        &registration,
        &object.props_json,
        &object.content_json,
    )
    .map_err(|error| format!("canonical_task_invalid:{}", error.pointer))
}

fn sync_entity<T: serde::Serialize>(
    entity_type: &str,
    id: &str,
    value: &T,
    hlc: &str,
) -> Result<SyncEntity, String> {
    let data = serde_json::to_value(value)
        .map_err(|error| error.to_string())?
        .as_object()
        .cloned()
        .unwrap_or_default();
    Ok(SyncEntity {
        entity_type: entity_type.into(),
        id: id.into(),
        data,
        hlc: hlc.into(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    })
}
