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
        value: TaskMergeValue::Field(Value::String(value.into())),
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
fn object_deletion_cannot_be_encoded_as_a_field_value() {
    let mut operation = op("00000000000000000000000000000001", &[], "value");
    operation.unit = TaskMergeUnit::ObjectExistence;

    assert_eq!(
        conflict_set(&[operation]),
        Err(MergeError::UnitValueMismatch)
    );
}
