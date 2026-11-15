//! The Engine's global hotkey, as this window sees it.
//!
//! The tap belongs to the Engine: it starts with the machine, runs with no
//! window open and keeps working after the interface is closed. This module
//! only reads what the Engine reports and forwards a capture it asked for, so
//! nothing here can take the hotkey down with the window.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use imago_nsh::ComponentContext;
use serde_json::{json, Value};

/// The page that mirrors the hotkey's state, once a page exists.
static PAGE: Mutex<Option<ComponentContext>> = Mutex::new(None);
/// The last answer from the Engine, so a repaint never blocks on a socket.
static STATUS: Mutex<Option<Value>> = Mutex::new(None);
/// How long a capture waits for a key before giving up, matching the Engine.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(30);
/// The Engine answers a status question in microseconds; asking four times a
/// second keeps the page's keycaps live without making a conversation of it.
const POLL: Duration = Duration::from_millis(250);

fn engine() -> crate::engine::Engine {
    crate::engine::Engine::new(None)
}

fn defaults() -> Value {
    json!({
        "running": false,
        "listenAccess": true,
        "capturing": false,
        "held": [],
    })
}

/// The pages take the hotkey's state with the rest of the snapshot.
pub fn status() -> Value {
    match STATUS.lock() {
        Ok(slot) => slot.clone().unwrap_or_else(defaults),
        Err(_) => defaults(),
    }
}

/// Starts mirroring the Engine: called once, when the pages create the bridge.
pub fn attach(context: ComponentContext) {
    if let Ok(mut page) = PAGE.lock() {
        *page = Some(context);
    }
    std::thread::spawn(|| loop {
        refresh();
        std::thread::sleep(POLL);
    });
}

/// The settings page asks for a combination; the Engine's hook reads the next
/// press and reports it back through `captured`.
pub fn capture() {
    let engine = engine();
    let _ = engine.rpc("dictation.begin_hotkey_capture", json!({}));
    std::thread::spawn(move || {
        let deadline = Instant::now() + CAPTURE_TIMEOUT;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(120));
            let Ok(answer) = engine.rpc("dictation.hotkey_status", json!({})) else {
                return;
            };
            if let Some(captured) = answer.get("captured").filter(|value| !value.is_null()) {
                let accelerator = captured.get("accelerator").and_then(Value::as_str);
                emit(
                    "hotkey",
                    match accelerator {
                        Some(accelerator) => json!({ "accelerator": accelerator }),
                        // The hook reports a cancel as an event without a key.
                        None => json!({ "cancelled": true, "reason": "cancelled" }),
                    },
                );
                return;
            }
            // A capture that stopped without an answer (Esc, or the page gave
            // up) leaves the stored combination alone.
            if answer["capturing"] != json!(true) {
                return;
            }
        }
    });
}

/// The page gave up waiting: the Engine stops listening for a combination.
pub fn cancel() {
    let _ = engine().rpc("dictation.end_hotkey_capture", json!({}));
}

/// macOS keeps the grant in the Input Monitoring pane, and the Engine is the
/// app that holds it, so the request is the Engine's to make.
pub fn request_access() {
    let _ = engine().rpc("dictation.hotkey_settings", json!({}));
}

fn refresh() {
    let Ok(answer) = engine().rpc("dictation.hotkey_status", json!({})) else {
        return;
    };
    let next = json!({
        "running": answer.get("running").cloned().unwrap_or(json!(false)),
        "listenAccess": answer.get("listenAccess").cloned().unwrap_or(json!(true)),
        "capturing": answer.get("capturing").cloned().unwrap_or(json!(false)),
        "held": answer.get("held").cloned().unwrap_or(json!([])),
    });
    let changed = match STATUS.lock() {
        Ok(mut slot) => {
            let changed = slot.as_ref() != Some(&next);
            if changed {
                *slot = Some(next.clone());
            }
            changed
        }
        Err(_) => false,
    };
    if !changed {
        return;
    }
    // The keycaps follow what is held right now; the page merges the rest from
    // the snapshot, so only these two move.
    emit(
        "hotkeyState",
        json!({
            "capturing": next["capturing"].clone(),
            "held": next["held"].clone(),
            "running": next["running"].clone(),
        }),
    );
}

fn emit(name: &str, data: Value) {
    if let Ok(page) = PAGE.lock() {
        if let Some(context) = page.as_ref() {
            context.emit(name, data);
        }
    }
}
