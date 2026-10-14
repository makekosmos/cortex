//! Host-side proof for KOS-370 unit 2 and KOS-397: open a temp data dir, upsert
//! an agenda task object in the `core.task` 0.1.0 shape agenda writes, list it
//! back, receive the `object_upserted` event through a `ChangeListener`, then
//! drop and reopen and still see the task. Legacy `com.kosmos.task` writes are
//! refused after the cutover, so this test writes the new shape only.

use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use engine_mobile::{ChangeListener, MobileEngine};
use serde_json::{json, Value};

const TASK_ID: &str = "task-mobile-roundtrip";

/// The object shape `agenda-gpui` writes: state and planning at `propsJson`
/// top level, UI-local fields under `propsJson.extensions`, notes duplicated
/// into `contentJson`.
fn task_object() -> Value {
    json!({
        "id": TASK_ID,
        "typeId": "core.task",
        "typeVersion": "0.1.0",
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
            "state": {"kind": "open", "at": null},
            "planning": {"kind": "date", "period": "any", "date": "2026-10-11"},
            "deadline": "2026-10-12",
            "reminders": [{"kind": "floating", "at": "2026-10-11T09:00:00.000Z", "timeZone": null}],
            "recurrence": null,
            "priority": "high",
            "checklist": [],
            "extensions": {
                "status": "todo",
                "created_at": "2026-10-10T12:00:00.000Z"
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

/// Deletes `path`, retrying for up to 10 s while Windows still refuses the
/// delete. `TempDir::drop` ignores that error, so a directory left behind
/// only shows up later as a gate leak with no test name; this fails here
/// instead, with the path, if the file never frees up.
fn remove_dir_when_free(path: &Path) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut result = std::fs::remove_dir_all(path);
    while result.is_err() && path.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        result = std::fs::remove_dir_all(path);
    }
    assert!(
        !path.exists(),
        "temp dir {} still in use: {result:?}",
        path.display()
    );
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
            json!({"type_id": "core.task"}),
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
        assert_eq!(
            fetched["propsJson"]["planning"]["date"],
            json!("2026-10-11")
        );
        assert_eq!(fetched["typeVersion"], json!("0.1.0"));

        engine.shutdown();
    }

    // Reopen on the same dir: the task must still be there.
    let engine = MobileEngine::open(data_dir).expect("reopen");
    let fetched = call(&engine, "get_object", json!({"id": TASK_ID}));
    assert_eq!(fetched["title"], json!("Call mom"));
    let listed = call(
        &engine,
        "list_objects_by_type",
        json!({"type_id": "core.task"}),
    );
    assert!(
        listed
            .as_array()
            .expect("list")
            .iter()
            .any(|o| o["id"] == TASK_ID),
        "task missing after reopen"
    );
    // Remove the dir here rather than at scope end: a lock that outlives
    // shutdown then fails this test by name instead of leaving a leftover.
    engine.shutdown();
    drop(engine);
    remove_dir_when_free(dir.path());
}
