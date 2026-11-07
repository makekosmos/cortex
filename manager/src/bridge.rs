//! The Manager's bridge to the pages: `manager.bridge` in TSX.
//!
//! The pages own no truth. They send commands, this side runs them against the
//! Engine and pushes the whole state back — the same shape Atlas uses, with the
//! Engine as the only source of facts.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{App, Window};
use imago_nsh::{ComponentContext, NativeComponent};
use serde_json::{json, Value};

use crate::boot;
use crate::data::Loader;
use crate::engine::{self, Engine};

/// The Engine operation that starts an installed program by its index id.
const APP_LAUNCH: &str = "app_index.launch";

/// Which page the sidebar is on. The pages ask for it and mirror what we say.
const ROUTES: [&str; 13] = [
    "server",
    "about",
    "appearance",
    "apps",
    "integrations",
    "storage",
    "activity",
    "devices",
    "browser",
    "keys",
    "models",
    "settings",
    "developer",
];

/// Routes whose data keeps moving while a job runs (a download, an update):
/// the page is re-read until the job says it is done.
const FOLLOW: [&str; 2] = ["models", "about"];

pub struct ManagerBridge {
    context: ComponentContext,
    /// Shared with the Engine poller thread, which writes what it reads.
    state: Arc<Mutex<Value>>,
    route: String,
    loader: Option<Loader>,
    started: bool,
}

impl ManagerBridge {
    pub fn new(context: ComponentContext) -> Self {
        let state = Arc::new(Mutex::new(snapshot()));
        // The Engine is the only source of facts, so it is watched from the
        // moment the pages create this component — not from the first frame:
        // a hidden or offscreen window may never draw one, and the pages would
        // then sit on their demo values.
        let engine = Engine::new(engine::data_dir());
        engine::watch(engine.clone(), context.clone(), state.clone());
        let loader = Loader::start(engine, context.clone(), state.clone());
        loader.open("server");
        Self {
            context,
            state,
            route: "server".into(),
            loader: Some(loader),
            started: false,
        }
    }

    fn publish(&self) {
        let state = self.state.lock().expect("the state lock").clone();
        self.context.emit("state", state);
    }

    /// Walks one `a.b` path of the state and writes what the page asked for.
    fn set(&mut self, path: &str, value: Value) {
        let mut state = self.state.lock().expect("the state lock");
        let mut cursor = &mut *state;
        for part in path.split('.').collect::<Vec<_>>() {
            if !cursor.is_object() {
                return;
            }
            cursor = cursor
                .as_object_mut()
                .expect("an object above")
                .entry(part)
                .or_insert(Value::Null);
        }
        *cursor = value;
    }

    fn status(&mut self, status: &str) {
        self.set("engine.status", json!(status));
        self.publish();
    }

    fn id(args: &Value) -> String {
        args.get("id")
            .or_else(|| args.get("value"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }

    /// Re-reads a page every couple of seconds for as long as the job runs, so
    /// a download shows progress instead of a frozen row.
    fn follow(&self, route: &str, loader: Loader, state: Arc<Mutex<Value>>) {
        let route = route.to_owned();
        std::thread::spawn(move || {
            for _ in 0..300 {
                std::thread::sleep(Duration::from_secs(2));
                loader.open(&route);
                let busy = is_busy(&route, &state);
                if !busy {
                    return;
                }
            }
        });
    }
}

/// A pairing ticket is opaque: the Engine answers a string or an object, and
/// the page shows whatever came back as text.
fn ticket(value: Value) -> Value {
    match value {
        Value::String(ticket) => json!(ticket),
        Value::Null => json!(""),
        other => json!(other.to_string()),
    }
}

/// Whether a page still has something in flight.
fn is_busy(route: &str, state: &Arc<Mutex<Value>>) -> bool {
    let state = match state.lock() {
        Ok(state) => state,
        Err(_) => return false,
    };
    match route {
        "models" => state["models"].as_array().is_some_and(|models| {
            models
                .iter()
                .any(|model| model["downloading"] == json!(true))
        }),
        "about" => {
            let percent = state["updates"]["percent"].as_f64().unwrap_or(0.0);
            let downloading = state["updates"]["state"]
                .as_str()
                .is_some_and(|state| state == "downloading");
            downloading && percent < 100.0
        }
        _ => false,
    }
}

impl NativeComponent for ManagerBridge {
    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> Option<gpui::AnyElement> {
        if !self.started {
            self.started = true;
            // Re-send the snapshot once the page has its handlers attached.
            self.publish();
        }
        None
    }

    fn set_prop(&mut self, name: &str, value: Option<&Value>, _w: &mut Window, _cx: &mut App) {
        if name != "route" {
            return;
        }
        let route = value
            .and_then(Value::as_str)
            .filter(|route| ROUTES.contains(route))
            .unwrap_or("server");
        self.route = route.to_owned();
        let route = self.route.clone();
        if let Some(loader) = &self.loader {
            loader.open(&route);
        }
        if FOLLOW.contains(&route.as_str()) {
            if let (Some(loader), true) = (&self.loader, is_busy(&route, &self.state)) {
                self.follow(&route, loader.clone(), self.state.clone());
            }
        }
        self.context.emit("navigate", json!({ "route": route }));
    }

    fn command(&mut self, name: &str, args: &Value, _window: &mut Window, _cx: &mut App) {
        let text = args.get("value").cloned().unwrap_or(Value::Null);
        let Some(loader) = self.loader.clone() else {
            return;
        };
        let flag = args.get("value").and_then(Value::as_bool).unwrap_or(false);
        match name {
            // The Engine's lifetime, off the UI thread. The pages ask for a
            // confirmation first; nothing is assumed here, the poller reports.
            "server.start" => {
                self.status("starting");
                std::thread::spawn(|| {
                    if let Err(error) = boot::ensure_engine() {
                        tracing::warn!(error = %error, "engine start failed");
                    }
                });
            }
            "server.stop" => {
                self.status("stopping");
                std::thread::spawn(|| {
                    if let Err(error) = boot::stop_engine() {
                        tracing::warn!(error = %error, "engine stop failed");
                    }
                });
            }

            // Pairing decisions (KOS-369): the Engine closes the handshake.
            "sync.accept" => loader.then_reload(
                "sync",
                "accept_pairing",
                json!({ "device_id": Self::id(args) }),
                "devices",
            ),
            "sync.decline" => loader.then_reload(
                "sync",
                "decline_pairing",
                json!({ "device_id": Self::id(args) }),
                "devices",
            ),
            // The code this computer shows: the Engine escalates the bind to a
            // LAN-reachable ticket only when asked for one.
            "pairing.code" => {
                loader.call("sync.ownCode", "show_pairing_code", json!({}), Some(ticket))
            }
            "connect.code" => loader.then_reload(
                "sync",
                "connect_with_pairing_code",
                json!({ "pairing_code": Self::id(args) }),
                "devices",
            ),
            "device.unlink" => loader.then_reload(
                "sync",
                "disconnect_peer",
                json!({ "device_id": Self::id(args) }),
                "devices",
            ),

            // Dictation: the Engine stores the key and never hands it back.
            "keys.save" => loader.then_reload(
                "keys",
                "dictation.set_api_key",
                json!({ "key": Self::id(args) }),
                "keys",
            ),
            "keys.verify" => loader.call(
                "keys",
                "dictation.verify_api_key",
                json!({ "key": Self::id(args) }),
                None,
            ),

            // Local dictation models.
            "models.download" => {
                loader.then_reload(
                    "models",
                    "dictation.download_local_model",
                    json!({ "modelId": Self::id(args), "select": true }),
                    "models",
                );
                self.follow("models", loader.clone(), self.state.clone());
            }
            "models.select" => loader.then_reload(
                "models",
                "dictation.update_config",
                json!({ "localModelId": Self::id(args) }),
                "models",
            ),
            "models.delete" => loader.then_reload(
                "models",
                "dictation.delete_local_model",
                json!({ "modelId": Self::id(args) }),
                "models",
            ),

            // Backups: create, then restore the one the user picked.
            "backups.create" => {
                loader.then_reload("storage", "manager.db_backups.create", json!({}), "storage")
            }
            "backups.restore" => loader.then_reload(
                "storage",
                "manager.db_backups.restore",
                json!({ "backup_id": Self::id(args) }),
                "storage",
            ),

            // Updates: status is read, the download and the install need a
            // click each — nothing downloads behind the user's back.
            "updates.check" => loader.then_reload("updates", "updater.check", json!({}), "about"),
            "updates.download" => {
                loader.then_reload("updates", "updater.download", json!({}), "about");
                self.follow("about", loader.clone(), self.state.clone());
            }
            "updates.install" => {
                loader.then_reload("updates", "updater.install", json!({}), "about")
            }

            // Appearance is device-wide and Engine-owned.
            "appearance.set" => {
                let patch = args.get("patch").cloned().unwrap_or_else(|| json!({}));
                loader.then_reload("appearance", "appearance.set", patch, "appearance");
            }

            // Switches the Engine owns: ask it, then show what it says.
            "settings.autostart" => loader.then_reload(
                "settings.autostart",
                "engine.autostart.set",
                json!({ "enabled": flag }),
                "settings",
            ),
            "settings.telemetry" => loader.then_reload(
                "settings",
                "engine.settings.set",
                json!({ "usage_tracker_enabled": flag }),
                "settings",
            ),

            // Connectors: the Engine syncs them; the Manager only asks.
            "connectors.syncNow" => loader.then_reload(
                "connectors",
                "integrations.sync_now",
                json!({}),
                "integrations",
            ),

            // «Открыть» on the apps page: the Engine launches the program from
            // its own index (`app_index.launch`), the Manager only asks.
            "app.open" => loader.call("apps", APP_LAUNCH, json!({ "id": Self::id(args) }), None),

            // Host-side facts the pages mirror.
            "client.name" => {
                self.set("client.name", text);
                self.publish();
            }
            "client.remoteControl" => {
                self.set("client.remoteControl", json!(flag));
                self.publish();
            }
            "appearance.new" | "connect.open" => {
                self.context.emit("navigate", json!({ "route": "devices" }));
            }
            _ => {}
        }
    }
}

/// What the pages see before the Engine answers. Every field is overwritten by
/// the first poll or page load.
fn snapshot() -> Value {
    json!({
        "engine": {
            "status": "unreachable",
            "version": "",
            "channel": "production",
            "port": 47321,
            "uptimeMinutes": 0,
        },
        "client": { "name": hostname(), "remoteControl": false },
        // The product version is the Engine's to report; the crate's own version
        // never stands in for it.
        "product": { "channel": "production", "bundleId": crate::identity::BUNDLE_ID },
        "sync": {
            "running": false,
            "pairingAvailable": false,
            "ownCodeAvailable": false,
            "incoming": [],
            "peers": [],
        },
        "devices": [],
        "apps": [],
        "models": [],
        "keys": { "hasApiKey": false, "engine": "", "language": "", "model": "" },
        "usage": { "todayMinutes": 0, "weekMinutes": 0, "top": [] },
        "storage": { "dataDir": data_directory(), "size": "", "lastBackup": "" },
        "backups": [],
        "connectors": [],
        "updates": {
            "state": "idle",
            "currentVersion": "",
            "newVersion": "",
            "channel": "",
            "canInstall": false,
            "percent": 0.0,
            "checkedAt": "",
        },
        "settings": {
            "autostart": false,
            "telemetry": false,
            "language": "Русский",
            "localOnly": true,
        },
        "appearance": {
            "mode": "dark",
            "light": "merino-light",
            "dark": "cobalt-foundry",
            "followApps": false,
            "accentSource": "theme",
            "accentColor": "",
        },
    })
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Этот компьютер".into())
}

pub fn data_directory() -> String {
    std::env::var("MUNDUS_DATA_DIR")
        .or_else(|_| std::env::var("APPDATA").map(|base| format!("{base}\\Mundus")))
        .unwrap_or_default()
}
