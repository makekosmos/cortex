use super::*;

fn op(id: &str, parents: &[&str], value: &str) -> TaskMergeOperation {
    TaskMergeOperation {
        id: OperationId::parse(id).unwrap(),
        object_id: "task-1".into(),
        unit: TaskMergeUnit::Title,
        parents: parents
            .iter()
            .map(|id| OperationId::parse(*id).unwrap())
            .collect(),
        observed_heads: parents
            .iter()
            .map(|id| OperationId::parse(*id).unwrap())
            .collect(),
        hlc: None,
        value: TaskMergeValue::Field(Value::String(value.into())),
    }
}

fn lifecycle_op(
    id: &str,
    unit: TaskMergeUnit,
    parents: &[&str],
    observed_heads: &[&str],
    value: TaskMergeValue,
) -> TaskMergeOperation {
    let ids = |values: &[&str]| {
        values
            .iter()
            .map(|id| OperationId::parse(*id).unwrap())
            .collect()
    };
    TaskMergeOperation {
        id: OperationId::parse(id).unwrap(),
        object_id: "task-1".into(),
        unit,
        parents: ids(parents),
        observed_heads: ids(observed_heads),
        hlc: None,
        value,
    }
}

#[test]
fn concurrent_values_keep_the_shared_base_and_have_order_independent_digest() {
    let base = op("00000000000000000000000000000001", &[], "base");
    let left = op(
        "00000000000000000000000000000002",
        &[base.id.as_str()],
        "left",
    );
    let right = op(
        "00000000000000000000000000000003",
        &[base.id.as_str()],
        "right",
    );
    let forward = conflict_set(&[base.clone(), left.clone(), right.clone()]).unwrap();
    let reverse = conflict_set(&[right, base.clone(), left]).unwrap();

    assert_eq!(forward.common_ancestors, vec![base]);
    assert_eq!(forward.variants.len(), 2);
    assert!(forward.has_conflict());
    assert_eq!(forward.digest, reverse.digest);
}

#[test]
fn legacy_operation_json_round_trips_without_new_causal_metadata() {
    let legacy = serde_json::json!({
        "id": "00000000000000000000000000000001",
        "objectId": "task-1",
        "unit": "title",
        "parents": [],
        "value": {"kind": "field", "value": "legacy"}
    });
    let operation: TaskMergeOperation = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(operation.observed_heads, Vec::<OperationId>::new());
    assert_eq!(operation.hlc, None);
    assert_eq!(serde_json::to_value(operation).unwrap(), legacy);
}

#[test]
fn operation_hlc_is_validated_but_does_not_replace_causal_parents() {
    let mut operation = op("00000000000000000000000000000001", &[], "value");
    operation.hlc = Some("not-an-hlc".into());
    assert_eq!(validate_operation(&operation), Err(MergeError::InvalidHlc));

    operation.hlc = Some("2026-10-17T00:00:00.000Z:000001:device".into());
    assert_eq!(validate_operation(&operation), Ok(()));

    let mut later_clock = op("00000000000000000000000000000002", &[], "later clock");
    later_clock.hlc = Some("2999-10-17T00:00:00.000Z:000001:device".into());
    let conflict = conflict_set(&[operation, later_clock]).unwrap();
    assert_eq!(conflict.variants.len(), 2);
    assert!(conflict.has_conflict());
}

#[test]
fn identical_concurrent_values_do_not_require_a_conflict_resolution() {
    let base = op("00000000000000000000000000000001", &[], "base");
    let left = op(
        "00000000000000000000000000000002",
        &[base.id.as_str()],
        "same",
    );
    let right = op(
        "00000000000000000000000000000003",
        &[base.id.as_str()],
        "same",
    );

    let conflicts = conflict_set(&[base, left, right]).unwrap();

    assert!(!conflicts.has_conflict());
}

#[test]
fn a_descendant_supersedes_only_the_parents_it_observed() {
    let base = op("00000000000000000000000000000001", &[], "base");
    let left = op(
        "00000000000000000000000000000002",
        &[base.id.as_str()],
        "left",
    );
    let right = op(
        "00000000000000000000000000000003",
        &[base.id.as_str()],
        "right",
    );
    let resolved = op(
        "00000000000000000000000000000004",
        &[left.id.as_str(), right.id.as_str()],
        "resolved",
    );
    let conflicts = conflict_set(&[base, left, right, resolved.clone()]).unwrap();

    assert_eq!(conflicts.variants, vec![resolved]);
}

#[test]
fn a_late_edit_from_one_branch_does_not_hide_the_other_branch() {
    let base = op("00000000000000000000000000000001", &[], "base");
    let left = op(
        "00000000000000000000000000000002",
        &[base.id.as_str()],
        "left",
    );
    let right = op(
        "00000000000000000000000000000003",
        &[base.id.as_str()],
        "right",
    );
    let left_update = op(
        "00000000000000000000000000000004",
        &[left.id.as_str()],
        "left updated",
    );

    let conflicts =
        conflict_set(&[base.clone(), left, right.clone(), left_update.clone()]).unwrap();

    assert_eq!(conflicts.common_ancestors, vec![base]);
    assert_eq!(conflicts.variants, vec![right, left_update]);
}

#[test]
fn dangling_parent_is_rejected_instead_of_becoming_an_unknown_base() {
    let child = op(
        "00000000000000000000000000000002",
        &["00000000000000000000000000000001"],
        "value",
    );
    assert_eq!(conflict_set(&[child]), Err(MergeError::MissingParent));
}

#[test]
fn one_operation_id_cannot_name_two_different_values() {
    let first = op("00000000000000000000000000000001", &[], "first");
    let replay_with_changed_bytes = op("00000000000000000000000000000001", &[], "second");

    assert_eq!(
        conflict_set(&[first, replay_with_changed_bytes]),
        Err(MergeError::OperationIdCollision)
    );
}

#[test]
fn parent_input_order_does_not_change_the_conflict_digest() {
    let base = op("00000000000000000000000000000001", &[], "base");
    let left = op(
        "00000000000000000000000000000002",
        &[base.id.as_str()],
        "left",
    );
    let right = op(
        "00000000000000000000000000000003",
        &[base.id.as_str()],
        "right",
    );
    let joined_one = op(
        "00000000000000000000000000000004",
        &[left.id.as_str(), right.id.as_str()],
        "joined",
    );
    let joined_two = op(
        "00000000000000000000000000000004",
        &[right.id.as_str(), left.id.as_str()],
        "joined",
    );

    assert_eq!(
        conflict_set(&[base.clone(), left.clone(), right.clone(), joined_one])
            .unwrap()
            .digest,
        conflict_set(&[base, left, right, joined_two])
            .unwrap()
            .digest
    );
}

#[test]
fn causal_cycles_are_rejected() {
    let left = op(
        "00000000000000000000000000000001",
        &["00000000000000000000000000000002"],
        "left",
    );
    let right = op(
        "00000000000000000000000000000002",
        &["00000000000000000000000000000001"],
        "right",
    );

    assert_eq!(conflict_set(&[left, right]), Err(MergeError::CausalCycle));
}

#[test]
fn deletion_is_a_distinct_concurrent_variant() {
    let mut base = op("00000000000000000000000000000001", &[], "unused");
    base.unit = TaskMergeUnit::ObjectExistence;
    base.value = TaskMergeValue::ObjectExists;
    let mut deletion = op(
        "00000000000000000000000000000002",
        &[base.id.as_str()],
        "unused",
    );
    deletion.unit = TaskMergeUnit::ObjectExistence;
    deletion.value = TaskMergeValue::ObjectDeleted {
        deleted_at: "2026-10-17T00:00:00Z".into(),
    };
    let mut edit = op(
        "00000000000000000000000000000003",
        &[base.id.as_str()],
        "unused",
    );
    edit.unit = TaskMergeUnit::ObjectExistence;
    edit.value = TaskMergeValue::ObjectExists;

    let conflicts = conflict_set(&[base.clone(), deletion.clone(), edit.clone()]).unwrap();
    assert_eq!(conflicts.common_ancestors, vec![base]);
    assert_eq!(conflicts.variants, vec![deletion, edit]);
}

#[test]
fn lifecycle_conflict_uses_cross_unit_observed_heads() {
    let title_base = lifecycle_op(
        "00000000000000000000000000000001",
        TaskMergeUnit::Title,
        &[],
        &[],
        TaskMergeValue::Field(Value::String("base".into())),
    );
    let existence_base = lifecycle_op(
        "00000000000000000000000000000002",
        TaskMergeUnit::ObjectExistence,
        &[],
        &[],
        TaskMergeValue::ObjectExists,
    );
    let deletion = lifecycle_op(
        "00000000000000000000000000000003",
        TaskMergeUnit::ObjectExistence,
        &[existence_base.id.as_str()],
        &[title_base.id.as_str(), existence_base.id.as_str()],
        TaskMergeValue::ObjectDeleted {
            deleted_at: "2026-10-17T00:00:00Z".into(),
        },
    );
    let edit = lifecycle_op(
        "00000000000000000000000000000004",
        TaskMergeUnit::Title,
        &[title_base.id.as_str()],
        &[title_base.id.as_str(), existence_base.id.as_str()],
        TaskMergeValue::Field(Value::String("edited".into())),
    );

    let conflicts = lifecycle_conflicts(&[
        title_base.clone(),
        existence_base.clone(),
        deletion.clone(),
        edit.clone(),
    ])
    .unwrap();
    assert_eq!(
        conflicts,
        vec![TaskLifecycleConflict {
            deletion: deletion.clone(),
            concurrent_head: edit.clone(),
        }]
    );

    let observed_deletion = lifecycle_op(
        "00000000000000000000000000000005",
        TaskMergeUnit::ObjectExistence,
        &[existence_base.id.as_str()],
        &[
            title_base.id.as_str(),
            existence_base.id.as_str(),
            edit.id.as_str(),
        ],
        TaskMergeValue::ObjectDeleted {
            deleted_at: "2026-10-17T00:00:01Z".into(),
        },
    );
    assert!(lifecycle_conflicts(&[
        title_base.clone(),
        existence_base.clone(),
        edit.clone(),
        observed_deletion.clone(),
    ])
    .unwrap()
    .is_empty());

    let edit_after_deletion = lifecycle_op(
        "00000000000000000000000000000007",
        TaskMergeUnit::Title,
        &[title_base.id.as_str()],
        &[
            title_base.id.as_str(),
            existence_base.id.as_str(),
            deletion.id.as_str(),
        ],
        TaskMergeValue::Field(Value::String("edited after delete".into())),
    );
    assert!(lifecycle_conflicts(&[
        title_base.clone(),
        existence_base.clone(),
        deletion.clone(),
        edit_after_deletion,
    ])
    .unwrap()
    .is_empty());

    let concurrent_deletion = lifecycle_op(
        "00000000000000000000000000000008",
        TaskMergeUnit::ObjectExistence,
        &[existence_base.id.as_str()],
        &[title_base.id.as_str(), existence_base.id.as_str()],
        TaskMergeValue::ObjectDeleted {
            deleted_at: "2026-10-17T00:00:02Z".into(),
        },
    );
    let deletion_conflict = conflict_set(&[
        existence_base.clone(),
        deletion.clone(),
        concurrent_deletion.clone(),
    ])
    .unwrap();
    assert_eq!(deletion_conflict.variants.len(), 2);
    assert!(deletion_conflict.has_conflict());

    let restore = lifecycle_op(
        "00000000000000000000000000000006",
        TaskMergeUnit::ObjectExistence,
        &[deletion.id.as_str()],
        &[
            title_base.id.as_str(),
            existence_base.id.as_str(),
            deletion.id.as_str(),
            edit.id.as_str(),
        ],
        TaskMergeValue::ObjectExists,
    );
    assert!(
        lifecycle_conflicts(&[title_base, existence_base, deletion, edit, restore,])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn object_deletion_cannot_be_encoded_as_a_field_value() {
    let mut operation = op("00000000000000000000000000000001", &[], "value");
    operation.unit = TaskMergeUnit::ObjectExistence;

    assert_eq!(
        conflict_set(&[operation]),
        Err(MergeError::UnitValueMismatch)
    );
}
