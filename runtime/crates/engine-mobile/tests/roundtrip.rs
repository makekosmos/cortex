//! Host-side proof for KOS-370 unit 2: open a temp data dir, upsert an
//! agenda task object in the same shape agenda-gpui writes
//! (com.kosmos.task@1.1.0, canonical props + `extensions`), list it back,
//! receive the `object_upserted` event through a `ChangeListener`, then
//! drop and reopen and still see the task.

use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use engine_mobile::{ChangeListener, MobileEngine};
use serde_json::{json, Value};

const TASK_ID: &str = "task-mobile-roundtrip";

/// The object shape `agenda-gpui` `src/store/mapping.rs` `write_at` emits:
/// canonical props at `propsJson` top level, UI-local fields under
/// `propsJson.extensions`, notes duplicated into `contentJson`.
fn task_object() -> Value {
    json!({
        "id": TASK_ID,
        "typeId": "com.kosmos.task",
        "typeVersion": "1.1.0",
        "title": "Call mom",
        "createdAt": "2026-10-10T12:00:00.000Z",
        "updatedAt": "2026-10-10T12:00:00.000Z",
        "deletedAt": null,
        "contentJson": {
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "notes body"}]
            }]
        },
        "propsJson": {
            "status": "todo",
            "priority": "high",
            "scheduledAt": "2026-10-11",
            "dueAt": "2026-10-12",
            "reminderAt": "2026-10-11T09:00:00.000Z",
            "completedAt": null,
            "canceledAt": null,
            "recurrence": null,
            "checklist": [],
            "extensions": {
                "status": "todo",
                "created_at": "2026-10-10T12:00:00.000Z",
                "priority": 2,
                "is_trashed": false
            }
        }
    })
}

struct Listener {
    tx: std::sync::mpsc::Sender<Value>,
}

impl ChangeListener for Listener {
    fn on_change(&self, event_json: String) {
        let _ = self
            .tx
            .send(serde_json::from_str(&event_json).unwrap_or(json!({})));
    }
}

fn call(engine: &MobileEngine, op: &str, params: Value) -> Value {
    let response: Value = serde_json::from_str(
        &engine
            .call(op.to_owned(), params.to_string())
            .expect("call transport"),
    )
    .expect("response json");
    assert!(
        response["ok"].as_bool().unwrap_or(false),
        "{op} rejected: {}",
        response["error"]
    );
    response["data"].clone()
}

fn wait_for_upsert(rx: &Receiver<Value>, id: &str) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => {
                if event["event"] == "object_upserted" && event["id"] == id {
                    return true;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if std::time::Instant::now() >= deadline {
                    return false;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return false,
        }
    }
}

#[test]
fn task_roundtrip_survives_reopen() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data_dir = dir.path().to_string_lossy().into_owned();

    let (tx, rx) = channel::<Value>();
    {
        let engine = MobileEngine::open(data_dir.clone()).expect("open");
        let _subscription = engine
            .subscribe(Box::new(Listener { tx: tx.clone() }))
            .expect("subscribe");

        let upserted = call(&engine, "upsert_object", json!({"object": task_object()}));
        assert_eq!(upserted, json!(true));
        assert!(
            wait_for_upsert(&rx, TASK_ID),
            "no object_upserted event within 10s"
        );

        let listed = call(
            &engine,
            "list_objects_by_type",
            json!({"type_id": "com.kosmos.task"}),
        );
        let ids: Vec<&str> = listed
            .as_array()
            .expect("list")
            .iter()
            .filter_map(|o| o["id"].as_str())
            .collect();
        assert!(ids.contains(&TASK_ID), "task missing from list: {listed}");

        let fetched = call(&engine, "get_object", json!({"id": TASK_ID}));
        assert_eq!(fetched["title"], json!("Call mom"));
        assert_eq!(fetched["propsJson"]["scheduledAt"], json!("2026-10-11"));
        assert_eq!(fetched["typeVersion"], json!("1.1.0"));

        engine.shutdown();
    }

    // Reopen on the same dir: the task must still be there.
    let engine = MobileEngine::open(data_dir).expect("reopen");
    let fetched = call(&engine, "get_object", json!({"id": TASK_ID}));
    assert_eq!(fetched["title"], json!("Call mom"));
    let listed = call(
        &engine,
        "list_objects_by_type",
        json!({"type_id": "com.kosmos.task"}),
    );
    assert!(
        listed
            .as_array()
            .expect("list")
            .iter()
            .any(|o| o["id"] == TASK_ID),
        "task missing after reopen"
    );
    engine.shutdown();
}
