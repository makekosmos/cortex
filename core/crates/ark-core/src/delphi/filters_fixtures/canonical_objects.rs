// Canonical fixture corpus for the Delphi filter parity contract.
use super::super::types::{SmartList, TodoItem};
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
        "delphi": {"sortOrder": spec.sort_order}
    });
    let props = json!({
        "state": {
            "kind": match spec.status {
                "done" => "done",
                "canceled" => "canceled",
                _ => "open",
            },
            "at": spec.completed.or(spec.canceled),
        },
        "planning": {
            "kind": if spec.someday {
                "someday"
            } else if spec.today {
                "today"
            } else if spec.scheduled.is_some() {
                "date"
            } else {
                "inbox"
            },
            "period": "any",
            "date": spec.scheduled,
        },
        "deadline": null,
        "reminders": [],
        "recurrence": null,
        "priority": "none",
        "checklist": [],
        "extensions": extensions
    });
    let object = ArkObject {
        id: spec.id.into(),
        type_id: "core.task".into(),
        type_version: "0.1.0".into(),
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

fn project_core_task_to_delphi(object: &ArkObject, links: &[ObjectLink]) -> TodoItem {
    let props = &object.props_json;
    let state = &props["state"];
    let planning = &props["planning"];
    let state_kind = state["kind"].as_str().unwrap_or_default();
    let planning_kind = planning["kind"].as_str().unwrap_or_default();
    let timestamp = state["at"].as_str().map(str::to_owned);
    TodoItem {
        id: object.id.clone(),
        title: object.title.clone(),
        scheduled_date: planning["date"].as_str().map(str::to_owned),
        is_today: planning_kind == "today",
        is_someday: planning_kind == "someday",
        is_completed: state_kind == "done",
        completed_at: if state_kind == "done" {
            timestamp.clone()
        } else {
            None
        },
        is_cancelled: state_kind == "canceled",
        cancelled_at: if state_kind == "canceled" {
            timestamp
        } else {
            None
        },
        is_trashed: object.deleted_at.is_some(),
        sort_order: props["extensions"]["delphi"]["sortOrder"]
            .as_i64()
            .unwrap_or_default(),
        created_at: object.created_at.clone(),
        project_id: links
            .iter()
            .find(|link| link.link_type == "project")
            .map(|link| link.target_object_id.clone()),
    }
}
