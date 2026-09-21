// Canonical fixture corpus for the Delphi filter parity contract.
use super::super::types::{SmartList, TodoItem};
use crate::canonical_types::compatibility::planning::project_task_to_delphi;
use crate::types::{ArkObject, ObjectLink};
use serde_json::json;

pub const TODAY_ISO: &str = "2026-05-15";

struct Spec {
    id: &'static str,
    status: &'static str,
    scheduled: Option<&'static str>,
    completed: Option<&'static str>,
    canceled: Option<&'static str>,
    today: bool,
    someday: bool,
    trashed: bool,
    sort_order: i64,
    created_at: &'static str,
    project: Option<&'static str>,
}

fn canonical(spec: &Spec) -> (ArkObject, Vec<ObjectLink>) {
    let extensions = json!({
        "compatibility": {"planning": {"isToday": spec.today, "sortOrder": spec.sort_order}},
        "kosmos": {"taskBucket": if spec.someday { "backlog" } else { "active" }}
    });
    let props = json!({
        "status": spec.status,
        "priority": "none",
        "scheduledAt": spec.scheduled,
        "dueAt": null,
        "reminderAt": null,
        "completedAt": spec.completed,
        "canceledAt": spec.canceled,
        "recurrence": null,
        "checklist": [],
        "extensions": extensions
    });
    let object = ArkObject {
        id: spec.id.into(),
        type_id: "com.kosmos.task".into(),
        type_version: "1.0.0".into(),
        title: spec.id.into(),
        content_json: json!({"type":"doc","content":[{"type":"paragraph"}]}),
        props_json: props,
        created_at: spec.created_at.into(),
        updated_at: spec.created_at.into(),
        deleted_at: spec.trashed.then_some("2026-05-15T12:00:00.000Z".into()),
    };
    let links = spec
        .project
        .map(|project| ObjectLink {
            id: format!("link-{}", spec.id),
            source_object_id: spec.id.into(),
            target_object_id: project.into(),
            link_type: "project".into(),
            created_at: spec.created_at.into(),
        })
        .into_iter()
        .collect();
    (object, links)
}

