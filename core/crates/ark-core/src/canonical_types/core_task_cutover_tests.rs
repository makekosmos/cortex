//! KOS-397 cutover: legacy task props map to core.task, a legacy task write is
//! refused, and a start moves the stored rows over once.
use super::core_task_rewrite_tests::{insert_legacy, legacy_props};
use crate::types::ArkObjectWrite;
use ark_application::canonical_compatibility::{
    map_legacy_with_context, LegacyRecord, LegacySource, MappingContext,
};
use rusqlite::Connection;
use serde_json::{json, Value};

fn map_flat(props: Value) -> Result<serde_json::Value, String> {
    let record = LegacyRecord {
        id: "task-1".into(),
        legacy_type_id: "task_obj".into(),
        title: "Задача".into(),
        content: json!({"type":"doc","content":[{"type":"paragraph"}]}),
        props,
        created_at: "2026-10-01T00:00:00Z".into(),
        updated_at: "2026-10-02T00:00:00Z".into(),
        deleted_at: None,
    };
    let raw = serde_json::to_vec(&record).unwrap();
    let mapped = map_legacy_with_context(
        LegacySource {
            source_kind: "task_obj",
            source_id: "task-1",
            raw_json: &raw,
        },
        &MappingContext {
            existing_object_ids: Default::default(),
            existing_links: Vec::new(),
        },
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(mapped.object.type_id, "core.task");
    assert_eq!(mapped.object.type_version, "0.1.0");
    Ok(mapped.object.props_json)
}

#[test]
fn flat_task_props_map_to_core_task_state_planning_and_recurrence() {
    let props = map_flat(json!({
        "status": "todo",
        "priority": 2,
        "is_today": true,
        "is_evening": true,
        "scheduled_date": "2026-10-14",
        "deadline": "2026-10-20",
        "reminder_date": "2026-10-19T08:00:00Z",
        "checklist_items": [{"id": "c1", "title": "Шаг", "isCompleted": true}],
        "recurrence_rule": {
            "frequency": "weekly",
            "interval": 2,
            "recurrenceType": "fixed",
            "daysOfWeek": [0, 2],
            "endDate": "2026-12-31T00:00:00Z"
        }
    }))
    .unwrap();
    assert_eq!(props["state"], json!({"kind": "open", "at": null}));
    // Today wins over the scheduled day, which stays in extensions.
    assert_eq!(
        props["planning"],
        json!({"kind": "today", "period": "evening", "date": null})
    );
    assert_eq!(
        props["extensions"]["compatibility"]["scheduledDate"],
        "2026-10-14"
    );
    assert_eq!(props["deadline"], "2026-10-20");
    assert_eq!(
        props["reminders"],
        json!([{"kind": "floating", "at": "2026-10-19T08:00:00Z", "timeZone": null}])
    );
    assert_eq!(props["priority"], "medium");
    assert_eq!(
        props["checklist"],
        json!([{"id": "c1", "text": "Шаг", "done": true, "rank": "000000"}])
    );
    // Legacy weekday 0 is Monday; core.task counts ISO days from 1.
    assert_eq!(
        props["recurrence"],
        json!({
            "kind": "calendar",
            "frequency": "weekly",
            "interval": 2,
            "daysOfWeek": [1, 3],
            "timeZone": null,
            "end": {"kind": "until", "date": "2026-12-31", "count": null}
        })
    );
}

#[test]
fn completion_keeps_its_stamp_and_a_stray_stamp_is_refused() {
    let props = map_flat(json!({
        "is_completed": true,
        "completed_at": "2026-10-11T14:53:27Z"
    }))
    .unwrap();
    assert_eq!(
        props["state"],
        json!({"kind": "done", "at": "2026-10-11T14:53:27Z"})
    );
    // A completion stamp on an open task would be dropped silently: refuse it.
    let stray = map_flat(json!({
        "status": "todo",
        "completed_at": "2026-10-11T14:53:27Z"
    }));
    assert!(stray.is_err());
}

#[test]
fn engine_refuses_a_new_write_of_the_legacy_task_type() {
    let conn = Connection::open_in_memory().unwrap();
    ark_store::open::init_schema(&conn).unwrap();
    let write = ArkObjectWrite {
        id: "new-legacy-task".into(),
        type_id: "com.kosmos.task".into(),
        type_version: Some("1.1.0".into()),
        title: "Old writer".into(),
        content_json: json!({"type":"doc","content":[{"type":"paragraph"}]}),
        props_json: json!({"status":"todo"}),
        created_at: "2026-10-13T00:00:00.000Z".into(),
        updated_at: "2026-10-13T00:00:00.000Z".into(),
        deleted_at: None,
    };
    let error = ark_store::object_write::prepare_object(&conn, write)
        .unwrap_err()
        .to_string();
    assert!(error.contains("legacy_task_type_retired"), "{error}");
}

#[tokio::test]
async fn start_moves_stored_legacy_tasks_once_and_keeps_a_backup() {
    let dir = crate::test_support::temp_dir();
    let path = dir.path().join("ark.db");
    {
        let conn = Connection::open(&path).unwrap();
        ark_store::open::init_schema(&conn).unwrap();
        insert_legacy(
            &conn,
            "legacy-task",
            &legacy_props("todo", true, false, false, false, false, None),
            false,
        )
        .unwrap();
    }
    let service = crate::service::ArkService::open(path.to_str().unwrap())
        .await
        .unwrap();
    let stored = service
        .call("get_object", json!({"id": "legacy-task"}))
        .await
        .unwrap();
    assert_eq!(stored["typeId"], "core.task");
    assert_eq!(stored["typeVersion"], "0.1.0");
    assert_eq!(stored["propsJson"]["planning"]["kind"], "today");
    drop(service);
    assert!(dir.path().join("ark.db.pre-core-task.bak").exists());

    // A second start finds nothing left to move and keeps the object as it is.
    let service = crate::service::ArkService::open(path.to_str().unwrap())
        .await
        .unwrap();
    let again = service
        .call("get_object", json!({"id": "legacy-task"}))
        .await
        .unwrap();
    assert_eq!(again["typeVersion"], "0.1.0");
}
