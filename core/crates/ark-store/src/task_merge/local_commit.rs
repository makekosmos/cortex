use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::task_merge::{
    value_for_unit, OperationId, TaskMergeBootstrap, TaskMergeOperation, TaskMergeUnit,
};
use ark_domain::types::{ArkObject, ObjectLink, SyncEntity};
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
    let state = super::load_local_mutation_state(connection, object_id)
        .map_err(|error| error.to_string())?;
    let mut object = state.object.ok_or_else(|| "MISSING_OBJECT".to_string())?;
    let normalized_device_id = normalized_device_id(device_id);
    let deleted_at = ark_domain::hlc::HLC::now(normalized_device_id).wall_time;
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

    let state = super::load_local_mutation_state(connection, &object.id)
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
    let previous_links = state.source_links.clone();

    let bootstrap = if !has_bootstrap {
        let baseline = previous.clone().unwrap_or_else(|| object.clone());
        let baseline_links = if previous.is_some() {
            previous_links.clone()
        } else {
            links.to_vec()
        };
        let source_hlc = previous
            .as_ref()
            .map(|_| {
                state.legacy_revision.clone().unwrap_or_else(|| {
                    ark_domain::hlc::HLC::now(normalized_device_id(device_id)).to_string()
                })
            })
            .unwrap_or_else(|| {
                ark_domain::hlc::HLC::now(normalized_device_id(device_id)).to_string()
            });
        let bootstrap = TaskMergeBootstrap {
            id: next_operation_id(),
            object: baseline,
            links: baseline_links,
            source_hlc,
            baseline_ids: TaskMergeUnit::ALL
                .into_iter()
                .map(|unit| (unit, next_operation_id()))
                .collect(),
        };
        Some(super::TaskMergeLocalBootstrapPlan { bootstrap })
    } else {
        None
    };

    let mut visible_heads = BTreeSet::new();
    let mut unit_heads = Vec::with_capacity(TaskMergeUnit::ALL.len());
    let unit_conflicts = if has_bootstrap {
        state.unit_conflicts.clone()
    } else {
        let Some(bootstrap) = bootstrap.as_ref() else {
            return Err("task_merge_bootstrap_missing_from_local_plan".into());
        };
        let bootstrap = bootstrap.bootstrap.clone();
        let baseline = bootstrap
            .baseline_operations()
            .map_err(|error| error.to_string())?;
        TaskMergeUnit::ALL
            .into_iter()
            .map(|unit| {
                let history = baseline
                    .iter()
                    .filter(|operation| operation.unit == unit)
                    .cloned()
                    .collect::<Vec<_>>();
                ark_domain::task_merge::conflict_set(&history)
                    .map(|set| (unit, set))
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    for (unit, set) in unit_conflicts {
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

    let mut operations = Vec::new();
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
            hlc: None,
            value: next_value,
        };
        let operation_id = operation.id.clone();
        let expected_conflict_digest = resolves_explicit_existence.then_some(set.digest.clone());
        operations.push(super::TaskMergeLocalOperation {
            operation,
            expected_conflict_digest,
        });
        visible_heads.insert(operation_id);
    }

    let result = super::apply_local_mutation(
        connection,
        super::TaskMergeLocalMutationPlan {
            expected_state: state,
            object: object.clone(),
            source_links: links.to_vec(),
            bootstrap,
            operations,
            device_id: device_id.to_owned(),
        },
    )
    .map_err(|error| error.to_string())?;
    if let Some(materialized) = &result.object {
        validate_materialized_task(materialized)?;
    }

    let entity = if let Some(materialized) = result.object.as_ref() {
        sync_entity(
            "object",
            &materialized.id,
            materialized,
            &result.object_revision,
        )?
    } else if result.hard_deleted {
        SyncEntity {
            entity_type: "object".into(),
            id: object.id.clone(),
            data: serde_json::json!({"typeId": "core.task"})
                .as_object()
                .cloned()
                .unwrap_or_default(),
            hlc: result.object_revision.clone(),
            deleted: Some(true),
            origin_device_id: Some(
                ark_domain::hlc::HLC::from_string(&result.object_revision).device_id,
            ),
            origin_seq: None,
        }
    } else {
        return Err("task_merge_materialized_object_missing".into());
    };

    let mut entities = vec![entity];
    if !result.hard_deleted {
        for link in links {
            let link_revision = result
                .link_revisions
                .iter()
                .find(|value| value.link_id == link.id)
                .ok_or_else(|| "task_merge_link_revision_missing".to_string())?;
            entities.push(sync_entity(
                "object_link",
                &link.id,
                link,
                &link_revision.revision,
            )?);
        }
    }
    Ok(entities)
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
    let registration = ark_domain::canonical_type::definitions::canonical_type_registrations()?
        .into_iter()
        .find(|registration| {
            registration.type_id == object.type_id && registration.version == object.type_version
        })
        .ok_or_else(|| "canonical_registry".to_string())?;
    ark_domain::canonical_type::validation::validate_canonical(
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
