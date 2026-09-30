#![allow(clippy::unwrap_used)]
use ark_core::canonical_types::compatibility::planning::project_task_to_delphi;
use ark_core::types::{ArkObject, ObjectLink};
use serde_json::json;

fn task(id: &str, props: serde_json::Value, deleted_at: Option<&str>) -> ArkObject {
    ArkObject {
        id: id.into(),
        type_id: "com.kosmos.task".into(),
        type_version: "1.0.0".into(),
        title: id.into(),
        content_json: json!({"type":"doc","content":[{"type":"paragraph"}]}),
        props_json: props,
        created_at: "2026-05-15T10:00:00.000Z".into(),
        updated_at: "2026-05-15T11:00:00.000Z".into(),
        deleted_at: deleted_at.map(str::to_owned),
    }
}

fn props(status: &str, extensions: serde_json::Value) -> serde_json::Value {
    json!({
        "status": status,
        "priority": "none",
        "scheduledAt": "2026-05-15T08:00:00.000Z",
        "dueAt": null,
        "reminderAt": null,
        "completedAt": if status == "done" { json!("2026-05-15T09:00:00.000Z") } else { json!(null) },
        "canceledAt": if status == "canceled" { json!("2026-05-15T09:30:00.000Z") } else { json!(null) },
        "recurrence": null,
        "checklist": [],
        "extensions": extensions,
    })
}

fn link(id: &str, target: &str, link_type: &str) -> ObjectLink {
    ObjectLink {
        id: id.into(),
        source_object_id: "t-1".into(),
        target_object_id: target.into(),
        link_type: link_type.into(),
        created_at: "2026-05-15T10:00:00.000Z".into(),
    }
}

#[test]
fn projects_canonical_task_fields_and_sole_project_link() {
    let object = task(
        "t-1",
        props(
            "done",
            json!({
                "compatibility": {"planning": {"isToday": true, "isEvening": true, "sortOrder": 42}},
                "kosmos": {"taskBucket": "backlog"}
            }),
        ),
        None,
    );
    let links = vec![link("p", "project-1", "project")];
    let view = project_task_to_delphi(&object, &links).unwrap();
    assert_eq!(view.id, "t-1");
    assert_eq!(
        view.scheduled_date.as_deref(),
        Some("2026-05-15T08:00:00.000Z")
    );
    assert!(view.is_today);
    assert!(view.is_someday);
    assert!(view.is_completed);
    assert_eq!(
        view.completed_at.as_deref(),
        Some("2026-05-15T09:00:00.000Z")
    );
    assert!(!view.is_cancelled);
    assert!(!view.is_trashed);
    assert_eq!(view.sort_order, 42);
    assert_eq!(view.created_at, object.created_at);
    assert_eq!(view.project_id.as_deref(), Some("project-1"));
}

#[test]
fn missing_optional_compatibility_values_use_view_defaults() {
    let object = task("t-1", props("todo", json!({})), None);
    let view = project_task_to_delphi(&object, &[]).unwrap();
    assert!(!view.is_today && !view.is_someday);
    assert_eq!(view.sort_order, 0);
    assert_eq!(view.project_id, None);
}

#[test]
fn malformed_flags_and_multiple_project_links_fail_stably() {
    let object = task(
        "t-1",
        props(
            "todo",
            json!({
                "compatibility": {"planning": {"isToday": "yes"}}
            }),
        ),
        None,
    );
    let error = project_task_to_delphi(&object, &[]).unwrap_err();
    assert_eq!(error.code(), "INVALID_FIELD");
    assert_eq!(
        error.pointer(),
        "/props/extensions/compatibility/planning/isToday"
    );

    let object = task("t-1", props("todo", json!({})), None);
    let error = project_task_to_delphi(
        &object,
        &[
            link("p1", "project-1", "project"),
            link("p2", "project-2", "project"),
        ],
    )
    .unwrap_err();
    assert_eq!(error.code(), "CONFLICTING_FIELDS");
    assert_eq!(error.pointer(), "/links/project");
}

#[test]
fn status_and_deleted_at_drive_view_flags_not_duplicate_top_level_props() {
    let object = task(
        "t-1",
        props(
            "canceled",
            json!({
                "isToday": true,
                "isCompleted": true,
                "isTrashed": false
            }),
        ),
        Some("2026-05-15T12:00:00.000Z"),
    );
    let view = project_task_to_delphi(&object, &[]).unwrap();
    assert!(!view.is_today && !view.is_completed);
    assert!(view.is_cancelled && view.is_trashed);
    assert_eq!(
        view.cancelled_at.as_deref(),
        Some("2026-05-15T09:30:00.000Z")
    );
}
