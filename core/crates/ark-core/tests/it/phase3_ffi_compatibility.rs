#![allow(clippy::unwrap_used)]
use ark_core::ffi::{ArkCore, ArkCoreError};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

fn opened_core() -> (tempfile::TempDir, Arc<ArkCore>, String) {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("ffi.sqlite");
    let path_string = path.to_string_lossy().into_owned();
    let core = ArkCore::new();
    assert!(core.open_db(path_string.clone()).expect("open db"));
    (dir, core, path_string)
}

fn todo_json(id: &str) -> String {
    serde_json::to_string(&json!({
        "id": id,
        "title": "From Android",
        "notes": null,
        "priority": 1,
        "scheduledDate": null,
        "deadline": null,
        "reminderDate": null,
        "isToday": false,
        "isEvening": false,
        "isSomeday": false,
        "isCompleted": false,
        "completedAt": null,
        "isCancelled": false,
        "cancelledAt": null,
        "isTrashed": false,
        "sortOrder": 0,
        "headingId": null,
        "projectId": null,
        "areaId": null,
        "tagIds": [],
        "checklistItems": [],
        "recurrenceRule": null,
        "createdAt": "2026-01-01T00:00:00.000Z"
    }))
    .expect("todo json")
}

#[test]
fn ffi_planning_writes_project_into_canonical_graph_without_legacy_rows() {
    let (_dir, core, path) = opened_core();
    let project = json!({
        "id": "project-ffi",
        "title": "Canonical project",
        "notes": null,
        "status": "active",
        "scheduledDate": null,
        "deadline": null,
        "sortOrder": 0,
        "colorTag": null,
        "areaId": null,
        "createdAt": "2026-01-01T00:00:00.000Z"
    });
    assert!(core
        .upsert_project_json(project.to_string())
        .expect("project write"));

    let conn = rusqlite::Connection::open(path).expect("inspect db");
    let canonical: (String, String) = conn
        .query_row(
            "SELECT type_id, type_version FROM objects WHERE id='project-ffi'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("canonical object");
    assert_eq!(canonical, ("com.kosmos.project".into(), "1.0.0".into()));
    let legacy_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(legacy_count, 0);
    let sync_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM object_sync_versions WHERE object_id='project-ffi'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sync_count, 1);
}

#[test]
fn ffi_area_and_heading_writes_are_read_only_without_side_effects() {
    let (_dir, core, path) = opened_core();
    let before = snapshot(&path);
    let area = core.upsert_area_json(
        json!({"id":"area-ffi","title":"Area","sortOrder":0,"createdAt":"2026-01-01"}).to_string(),
    );
    assert!(area.is_err());
    assert_structured_error(
        area.unwrap_err(),
        "compatibility",
        "LEGACY_PLANNING_READ_ONLY",
    );
    let heading = core.upsert_heading_json(
        json!({"id":"heading-ffi","title":"Heading","sortOrder":0,"projectId":"p"}).to_string(),
    );
    assert!(heading.is_err());
    assert_structured_error(
        heading.unwrap_err(),
        "compatibility",
        "LEGACY_PLANNING_READ_ONLY",
    );
    let delete = core.delete_heading("heading-ffi".into());
    assert_structured_error(
        delete.unwrap_err(),
        "compatibility",
        "LEGACY_PLANNING_READ_ONLY",
    );
    assert_eq!(before, snapshot(&path));
}

#[test]
fn ffi_invalid_broadcast_is_rejected_before_hlc_or_storage() {
    let (_dir, core, path) = opened_core();
    let invalid = json!({
        "type": "object", "id": "bad-ffi", "data": {
            "typeId": "com.kosmos.note", "typeVersion": "1.0.0", "type":"note", "title": "bad",
            "contentJson": {"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"bad","marks":[{"type":"unknown"}]}]}]}, "propsJson": {"description":"bad","extensions":{"compatibility":{}}},
            "createdAt": "2026-01-01", "updatedAt": "2026-01-01", "deletedAt": null
        }, "hlc": "", "deleted": null
    });
    let before = snapshot(&path);
    assert_structured_error(
        core.broadcast_change_json(invalid.to_string()).unwrap_err(),
        "compatibility",
        "DATA_LOSS_RISK",
    );
    assert_eq!(before, snapshot(&path));
}

fn assert_structured_error(error: ArkCoreError, category: &str, code: &str) {
    match error {
        ArkCoreError::Generic(message) => {
            let value: serde_json::Value =
                serde_json::from_str(&message).expect("structured FFI error");
            assert_eq!(value["category"], category);
            assert_eq!(value["code"], code);
        }
    }
}

fn snapshot(path: &str) -> (u64, Vec<(String, i64)>) {
    let conn = rusqlite::Connection::open(path).expect("inspect db");
    let tables = [
        "objects",
        "object_links",
        "object_sync_versions",
        "sync_kv",
        "events",
    ];
    let counts = tables
        .iter()
        .map(|table| {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap_or(0);
            ((*table).to_string(), count)
        })
        .collect();
    (std::fs::metadata(path).expect("db").len(), counts)
}

#[test]
fn ffi_android_todo_wire_shape_is_accepted_at_byte_boundary() {
    let (_dir, core, path) = opened_core();
    assert!(core
        .upsert_todo_json(todo_json("todo-ffi"))
        .expect("todo write"));
    let conn = rusqlite::Connection::open(path).expect("inspect db");
    let canonical: String = conn
        .query_row("SELECT type_id FROM objects WHERE id='todo-ffi'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(canonical, "com.kosmos.task");
}
