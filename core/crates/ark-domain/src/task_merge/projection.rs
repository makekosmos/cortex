use super::{MergeError, TaskConflictSet, TaskMergeUnit, TaskMergeValue};
use crate::types::ArkObject;
use serde_json::Value;
use std::collections::BTreeSet;

pub fn value_for_unit(
    object: &ArkObject,
    unit: TaskMergeUnit,
) -> Result<TaskMergeValue, MergeError> {
    let value = match unit {
        TaskMergeUnit::ObjectExistence => {
            return Ok(match &object.deleted_at {
                Some(deleted_at) => TaskMergeValue::ObjectDeleted {
                    deleted_at: deleted_at.clone(),
                },
                None => TaskMergeValue::ObjectExists,
            });
        }
        TaskMergeUnit::Title => Value::String(object.title.clone()),
        TaskMergeUnit::Content => object.content_json.clone(),
        unit => {
            let property = property_for_unit(unit).ok_or(MergeError::UnitValueMismatch)?;
            let props = object
                .props_json
                .as_object()
                .ok_or(MergeError::InvalidTaskPayload)?;
            let Some(value) = props.get(property) else {
                return Ok(TaskMergeValue::FieldAbsent);
            };
            value.clone()
        }
    };
    Ok(TaskMergeValue::Field(value))
}

pub fn apply_unit_value(
    object: &mut ArkObject,
    unit: TaskMergeUnit,
    value: &TaskMergeValue,
) -> Result<(), MergeError> {
    match (unit, value) {
        (TaskMergeUnit::ObjectExistence, TaskMergeValue::ObjectExists) => {
            object.deleted_at = None;
        }
        (TaskMergeUnit::ObjectExistence, TaskMergeValue::ObjectDeleted { deleted_at }) => {
            object.deleted_at = Some(deleted_at.clone())
        }
        (TaskMergeUnit::Title, TaskMergeValue::Field(Value::String(title))) => {
            object.title = title.clone();
        }
        (TaskMergeUnit::Content, TaskMergeValue::Field(content)) => {
            object.content_json = content.clone();
        }
        (TaskMergeUnit::Title | TaskMergeUnit::Content, TaskMergeValue::FieldAbsent) => {
            return Err(MergeError::UnitValueMismatch);
        }
        (unit, TaskMergeValue::Field(value)) => {
            let property = property_for_unit(unit).ok_or(MergeError::UnitValueMismatch)?;
            object
                .props_json
                .as_object_mut()
                .ok_or(MergeError::InvalidTaskPayload)?
                .insert(property.to_owned(), value.clone());
        }
        (unit, TaskMergeValue::FieldAbsent) => {
            let property = property_for_unit(unit).ok_or(MergeError::UnitValueMismatch)?;
            object
                .props_json
                .as_object_mut()
                .ok_or(MergeError::InvalidTaskPayload)?
                .remove(property);
        }
        _ => return Err(MergeError::UnitValueMismatch),
    }
    Ok(())
}

/// Materializes a deterministic visible task from each unit's current heads.
/// Concurrent variants remain in the conflict sets for the caller to expose.
pub fn materialize_task(
    object: &ArkObject,
    conflict_sets: &[TaskConflictSet],
) -> Result<ArkObject, MergeError> {
    let mut materialized = object.clone();
    let mut seen_units = BTreeSet::new();

    for conflict_set in conflict_sets {
        if conflict_set.object_id != object.id {
            return Err(MergeError::MixedHistory);
        }
        if !seen_units.insert(conflict_set.unit) {
            return Err(MergeError::DuplicateMaterializedUnit);
        }
        let winner = conflict_set
            .variants
            .iter()
            .min_by(|left, right| left.id.cmp(&right.id))
            .ok_or(MergeError::EmptyHistory)?;
        apply_unit_value(&mut materialized, conflict_set.unit, &winner.value)?;
    }

    Ok(materialized)
}

fn property_for_unit(unit: TaskMergeUnit) -> Option<&'static str> {
    match unit {
        TaskMergeUnit::State => Some("state"),
        TaskMergeUnit::Planning => Some("planning"),
        TaskMergeUnit::Deadline => Some("deadline"),
        TaskMergeUnit::Reminders => Some("reminders"),
        TaskMergeUnit::Recurrence => Some("recurrence"),
        TaskMergeUnit::Priority => Some("priority"),
        TaskMergeUnit::Checklist => Some("checklist"),
        TaskMergeUnit::Title | TaskMergeUnit::Content | TaskMergeUnit::ObjectExistence => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_merge::TaskMergeUnit;
    use serde_json::json;

    fn task() -> ArkObject {
        ArkObject {
            id: "task-1".into(),
            type_id: "core.task".into(),
            type_version: "0.1.0".into(),
            title: "before".into(),
            content_json: json!({"type":"doc","content":[]}),
            props_json: json!({
                "state": {"kind":"open","at":null},
                "planning": {"kind":"inbox","period":"any","date":null},
                "deadline": null,
                "reminders": [],
                "recurrence": null,
                "priority": 0,
                "checklist": [],
                "extensions": {}
            }),
            created_at: "2026-10-17T00:00:00Z".into(),
            updated_at: "2026-10-17T00:00:00Z".into(),
            deleted_at: None,
        }
    }

    #[test]
    fn each_merge_unit_round_trips_through_the_materialized_task() {
        let mut object = task();
        for unit in [
            TaskMergeUnit::Title,
            TaskMergeUnit::Content,
            TaskMergeUnit::State,
            TaskMergeUnit::Planning,
            TaskMergeUnit::Deadline,
            TaskMergeUnit::Reminders,
            TaskMergeUnit::Recurrence,
            TaskMergeUnit::Priority,
            TaskMergeUnit::Checklist,
            TaskMergeUnit::ObjectExistence,
        ] {
            let value = value_for_unit(&object, unit).unwrap();
            let before = object.clone();
            apply_unit_value(&mut object, unit, &value).unwrap();
            assert_eq!(
                value_for_unit(&object, unit).unwrap(),
                value,
                "unit {unit:?} changed while being materialized"
            );
            object = before;
        }
    }

    #[test]
    fn field_absence_is_distinct_from_json_null() {
        let mut object = task();
        let null = TaskMergeValue::Field(Value::Null);
        apply_unit_value(&mut object, TaskMergeUnit::Deadline, &null).unwrap();
        assert_eq!(
            value_for_unit(&object, TaskMergeUnit::Deadline).unwrap(),
            TaskMergeValue::Field(Value::Null)
        );

        apply_unit_value(
            &mut object,
            TaskMergeUnit::Deadline,
            &TaskMergeValue::FieldAbsent,
        )
        .unwrap();
        assert_eq!(
            value_for_unit(&object, TaskMergeUnit::Deadline).unwrap(),
            TaskMergeValue::FieldAbsent
        );
    }

    #[test]
    fn object_lifecycle_is_materialized_without_changing_field_values() {
        let mut object = task();
        let title = object.title.clone();
        apply_unit_value(
            &mut object,
            TaskMergeUnit::ObjectExistence,
            &TaskMergeValue::ObjectDeleted {
                deleted_at: "2026-10-17T01:00:00Z".into(),
            },
        )
        .unwrap();
        assert_eq!(
            value_for_unit(&object, TaskMergeUnit::ObjectExistence).unwrap(),
            TaskMergeValue::ObjectDeleted {
                deleted_at: "2026-10-17T01:00:00Z".into()
            }
        );
        assert_eq!(object.title, title);
    }

    #[test]
    fn materialization_rejects_invalid_field_and_payload_shapes() {
        let mut object = task();
        assert_eq!(
            apply_unit_value(
                &mut object,
                TaskMergeUnit::Title,
                &TaskMergeValue::FieldAbsent
            ),
            Err(MergeError::UnitValueMismatch)
        );
        object.props_json = json!(null);
        assert_eq!(
            value_for_unit(&object, TaskMergeUnit::Priority),
            Err(MergeError::InvalidTaskPayload)
        );
        assert_eq!(
            apply_unit_value(
                &mut object,
                TaskMergeUnit::Priority,
                &TaskMergeValue::Field(json!(0))
            ),
            Err(MergeError::InvalidTaskPayload)
        );
    }

    #[test]
    fn materialization_chooses_a_stable_visible_variant_and_preserves_conflicts() {
        let operation = |id: char, value: &str| super::super::TaskMergeOperation {
            id: super::super::OperationId::parse(format!("{}{}", "0".repeat(31), id)).unwrap(),
            object_id: "task-1".into(),
            unit: TaskMergeUnit::Title,
            parents: Vec::new(),
            value: TaskMergeValue::Field(Value::String(value.into())),
        };
        let conflict_set =
            super::super::conflict_set(&[operation('2', "second"), operation('1', "first")])
                .unwrap();

        let object = materialize_task(&task(), std::slice::from_ref(&conflict_set)).unwrap();

        assert_eq!(object.title, "first");
        assert!(conflict_set.has_conflict());
        assert_eq!(conflict_set.variants.len(), 2);
    }

    #[test]
    fn materialization_rejects_sets_for_another_task_or_duplicate_units() {
        let operation = super::super::TaskMergeOperation {
            id: super::super::OperationId::parse("00000000000000000000000000000001").unwrap(),
            object_id: "other-task".into(),
            unit: TaskMergeUnit::Title,
            parents: Vec::new(),
            value: TaskMergeValue::Field(Value::String("other".into())),
        };
        let conflict_set = super::super::conflict_set(&[operation]).unwrap();
        assert!(matches!(
            materialize_task(&task(), std::slice::from_ref(&conflict_set)),
            Err(MergeError::MixedHistory)
        ));
        let task_operation = super::super::TaskMergeOperation {
            id: super::super::OperationId::parse("00000000000000000000000000000002").unwrap(),
            object_id: "task-1".into(),
            unit: TaskMergeUnit::Title,
            parents: Vec::new(),
            value: TaskMergeValue::Field(Value::String("task".into())),
        };
        let task_set = super::super::conflict_set(&[task_operation]).unwrap();
        assert!(matches!(
            materialize_task(&task(), &[task_set.clone(), task_set]),
            Err(MergeError::DuplicateMaterializedUnit)
        ));
    }
}
