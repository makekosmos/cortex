//! Canonical fixture corpus for the Delphi filter parity contract.
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

pub fn fixtures() -> Vec<TodoItem> {
    let specs = [
        Spec {
            id: "inbox-1",
            status: "todo",
            scheduled: None,
            completed: None,
            canceled: None,
            today: false,
            someday: false,
            trashed: false,
            sort_order: 1,
            created_at: "2026-05-13T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "inbox-2",
            status: "todo",
            scheduled: None,
            completed: None,
            canceled: None,
            today: false,
            someday: false,
            trashed: false,
            sort_order: 2,
            created_at: "2026-05-14T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "today-flag",
            status: "todo",
            scheduled: None,
            completed: None,
            canceled: None,
            today: true,
            someday: false,
            trashed: false,
            sort_order: 10,
            created_at: "2026-05-15T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "today-scheduled",
            status: "todo",
            scheduled: Some(TODAY_ISO),
            completed: None,
            canceled: None,
            today: false,
            someday: false,
            trashed: false,
            sort_order: 5,
            created_at: "2026-05-15T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "upcoming-1",
            status: "todo",
            scheduled: Some("2026-06-01"),
            completed: None,
            canceled: None,
            today: false,
            someday: false,
            trashed: false,
            sort_order: 100,
            created_at: "2026-05-10T00:00:00.000Z",
            project: None,
        },
        Spec {
            id: "upcoming-2",
            status: "todo",
            scheduled: Some("2026-05-20"),
            completed: None,
            canceled: None,
            today: false,
            someday: false,
            trashed: false,
            sort_order: 99,
            created_at: "2026-05-11T00:00:00.000Z",
            project: None,
        },
        Spec {
            id: "someday-1",
            status: "todo",
            scheduled: None,
            completed: None,
            canceled: None,
            today: false,
            someday: true,
            trashed: false,
            sort_order: 50,
            created_at: "2026-05-15T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "someday-2",
            status: "todo",
            scheduled: None,
            completed: None,
            canceled: None,
            today: false,
            someday: true,
            trashed: false,
            sort_order: 51,
            created_at: "2026-05-15T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "proj-task",
            status: "todo",
            scheduled: Some("2026-05-20"),
            completed: None,
            canceled: None,
            today: false,
            someday: false,
            trashed: false,
            sort_order: 7,
            created_at: "2026-05-15T10:00:00.000Z",
            project: Some("proj-1"),
        },
        Spec {
            id: "completed-1",
            status: "done",
            scheduled: None,
            completed: Some("2026-05-14T12:00:00.000Z"),
            canceled: None,
            today: false,
            someday: false,
            trashed: false,
            sort_order: 0,
            created_at: "2026-05-15T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "completed-2",
            status: "done",
            scheduled: None,
            completed: Some("2026-05-15T08:00:00.000Z"),
            canceled: None,
            today: false,
            someday: false,
            trashed: false,
            sort_order: 0,
            created_at: "2026-05-15T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "cancelled-1",
            status: "canceled",
            scheduled: None,
            completed: None,
            canceled: Some("2026-05-13T15:00:00.000Z"),
            today: false,
            someday: false,
            trashed: false,
            sort_order: 0,
            created_at: "2026-05-15T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "trash-1",
            status: "todo",
            scheduled: None,
            completed: None,
            canceled: None,
            today: false,
            someday: false,
            trashed: true,
            sort_order: 0,
            created_at: "2026-05-12T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "trash-2",
            status: "todo",
            scheduled: None,
            completed: None,
            canceled: None,
            today: false,
            someday: false,
            trashed: true,
            sort_order: 0,
            created_at: "2026-05-15T11:00:00.000Z",
            project: None,
        },
        Spec {
            id: "scheduled-today-completed",
            status: "done",
            scheduled: Some(TODAY_ISO),
            completed: Some("2026-05-15T10:00:00.000Z"),
            canceled: None,
            today: false,
            someday: false,
            trashed: false,
            sort_order: 0,
            created_at: "2026-05-15T10:00:00.000Z",
            project: None,
        },
        Spec {
            id: "trashed-completed",
            status: "done",
            scheduled: None,
            completed: Some("2026-05-14T10:00:00.000Z"),
            canceled: None,
            today: false,
            someday: false,
            trashed: true,
            sort_order: 0,
            created_at: "2026-05-15T10:00:00.000Z",
            project: None,
        },
    ]
    .into_iter()
    .map(|spec| {
        let (object, links) = canonical(&spec);
        project_task_to_delphi(&object, &links).expect("canonical fixture must project")
    })
    .collect();
    specs
}

pub fn expected_filter_output(list: SmartList) -> Vec<String> {
    let ids: &[&str] = match list {
        SmartList::Inbox => &[
            "inbox-1",
            "inbox-2",
            "today-scheduled",
            "today-flag",
            "upcoming-2",
            "upcoming-1",
        ],
        SmartList::Today => &["today-scheduled", "today-flag"],
        SmartList::Upcoming => &["today-scheduled", "upcoming-2", "proj-task", "upcoming-1"],
        SmartList::Anytime => &[
            "inbox-1",
            "inbox-2",
            "today-scheduled",
            "proj-task",
            "today-flag",
            "upcoming-2",
            "upcoming-1",
        ],
        SmartList::Someday => &["someday-1", "someday-2"],
        SmartList::Logbook => &[
            "scheduled-today-completed",
            "completed-2",
            "completed-1",
            "trashed-completed",
            "cancelled-1",
        ],
        SmartList::Trash => &["trash-2", "trashed-completed", "trash-1"],
    };
    ids.iter().map(|id| (*id).into()).collect()
}

pub fn expected_counts() -> [usize; 7] {
    [6, 2, 4, 7, 2, 5, 3]
}
