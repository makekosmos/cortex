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

/// Which page the sidebar is on. The pages ask for it and mirror what we say.
const ROUTES: [&str; 12] = [
    "server",
    "about",
    "appearance",
    "plugins",
    "integrations",
    "storage",
    "activity",
    "devices",
    "browser",
    "keys",
    "models",
    "settings",
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
        // The sidebar lists installed plugins on every page, not only on the
        // one that installs them, so that is read from the start.
        loader.open("plugins");
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

/// A page the sidebar can open: one of the listed routes, or a plugin's own.
fn is_route(route: &str) -> bool {
    ROUTES.contains(&route) || route.starts_with("plugin/")
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
            .filter(|route| is_route(route))
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

    /// A plugin's own page, `plugin/<id>`: the id is the plugin's, so the
    /// route is built rather than listed.
    fn command(&mut self, name: &str, args: &Value, window: &mut Window, cx: &mut App) {
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

            // The rest of the page's presses move it, not the Engine.
            "connect.open" => self.context.emit("navigate", json!({ "route": "devices" })),

            // Plugins: the Engine installs from the catalog and keeps the
            // settings, so every one of these is a call and a re-read.
            // A download is one blocking call: the page shows its press as
            // spent and reads the list again when the Engine answers.
            "plugins.install" => loader.then_reload(
                "plugins.installed",
                "plugins.install",
                json!({ "id": Self::id(args) }),
                "plugins",
            ),
            "plugins.remove" => loader.then_reload(
                "plugins.installed",
                "plugins.remove",
                json!({ "id": Self::id(args) }),
                "plugins",
            ),
            "plugins.launch" => loader.then_reload(
                "plugins.installed",
                "plugins.launch",
                json!({ "id": Self::id(args) }),
                "plugins",
            ),
            // Whether the Engine starts this plugin on boot: the answer lands
            // under a key of its own, and the list is read again for the row.
            "plugins.autostart" => loader.then_reload(
                "plugins.autostart",
                "plugins.autostart.set",
                json!({
                    "id": Self::id(args),
                    "enabled": args.get("value").and_then(Value::as_bool).unwrap_or(false),
                }),
                "plugins",
            ),
            // The page that sent this is the plugin's own (`plugin/<id>`).
            "plugins.set" => loader.then_reload(
                "plugins.current",
                "plugins.settings.set",
                json!({
                    "id": Self::id(args),
                    "values": args.get("values").cloned().unwrap_or_else(|| json!({})),
                }),
                &self.route.clone(),
            ),

            // The window's bar and its sidebar: a press there moves the window,
            // which the host has to start itself (`chrome::start_move`).
            "window.drag" => crate::chrome::start_move(window, cx),
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
            // A port appears when an Engine answers; the default Engine port is
            // not a fact the Manager may state.
            "port": 0,
        },
        "client": { "name": hostname() },
        // The product version is the Engine's to report; the crate's own version
        // never stands in for it.
        "product": { "bundleId": crate::identity::BUNDLE_ID },
        "sync": { "running": false, "incoming": [], "peers": [] },
        "models": [],
        "keys": { "hasApiKey": false, "engine": "", "language": "", "model": "" },
        "usage": { "todayMinutes": 0, "weekMinutes": 0, "top": [] },
        "storage": { "dataDir": data_directory(), "size": "" },
        "backups": [],
        "connectors": [],
        // Until the Engine answers: nothing to install, nothing installed. The
        // page reads these two as "не удалось получить список".
        "plugins": {
            "catalog": { "available": [], "stale": false, "url": "" },
            "installed": [],
            "current": null,
        },
        "updates": {
            "state": "idle",
            "currentVersion": "",
            "newVersion": "",
            "channel": "",
            "canInstall": false,
            "percent": 0.0,
        },
        "settings": { "autostart": false, "telemetry": false },
        "appearance": {
            "mode": "dark",
            "light": "merino-light",
            "dark": "cobalt-foundry",
            "followApps": false,
            "accentSource": "theme",
        },
    })
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Этот компьютер".into())
}

/// Where the Engine keeps its data, resolved the way the Engine resolves it, so
/// the storage page names the same directory a running Engine writes to.
pub fn data_directory() -> String {
    crate::engine::data_home()
        .map(|dir| dir.display().to_string())
        .unwrap_or_default()
}
