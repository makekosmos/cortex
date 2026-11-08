//! The Engine's HTTP surface: lock discovery, status reads and `/v1/rpc`.
//!
//! The pages never touch it. They send a command to the bridge; this side calls
//! the Engine and pushes the resulting state back down. Discovery reads
//! `engine.lock.json` on every call, so an Engine restarted under a new port is
//! picked up on the next poll without the Manager noticing.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use imago_nsh::ComponentContext;
use serde::Deserialize;
use serde_json::{json, Value};

/// The Engine's wire protocol still carries its pre-rename names for the API
/// version and the client pid, and running Engines accept only those — the
/// client *class* header is the neutral one (`identity::CLIENT_CLASS_HEADER`),
/// so a product rename never reaches the protocol.
const API_VERSION: &str = "1.0.0";

/// How often the Engine is asked whether it is there.
const POLL: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Deserialize)]
struct Lock {
    format_version: u32,
    api_version: ApiVersion,
    http_port: u16,
    auth_token: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ApiVersion {
    major: u32,
}

#[derive(Clone, Debug)]
pub struct Engine {
    data_dir: Option<PathBuf>,
}

impl Engine {
    pub fn new(data_dir: Option<PathBuf>) -> Self {
        Self { data_dir }
    }

    /// The running Engine's port and token, re-read each time: the file is the
    /// only place that knows, and it changes when the Engine restarts.
    fn lock(&self) -> Result<Lock, String> {
        let directory = match &self.data_dir {
            Some(dir) => dir.clone(),
            None => data_dir().ok_or_else(|| "no data directory resolves".to_owned())?,
        };
        let bytes = std::fs::read(directory.join("engine.lock.json"))
            .map_err(|error| format!("engine.lock.json unreadable: {error}"))?;
        let lock: Lock = serde_json::from_slice(&bytes)
            .map_err(|error| format!("engine.lock.json malformed: {error}"))?;
        if lock.format_version != 1
            || lock.api_version.major != 1
            || lock.http_port == 0
            || lock.auth_token.len() != 64
            || !lock.auth_token.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("engine.lock.json: incompatible format".to_owned());
        }
        Ok(lock)
    }

    /// GET /v1/health or /v1/info — the reachability surfaces.
    pub fn status(&self, path: &str) -> Result<Value, String> {
        let lock = self.lock()?;
        let response = self
            .agent()
            .get(&format!("http://127.0.0.1:{}/v1/{path}", lock.http_port))
            .set("Authorization", &format!("Bearer {}", lock.auth_token))
            .set("X-Kosmos-Api-Version", API_VERSION)
            .set(
                crate::identity::CLIENT_CLASS_HEADER,
                crate::identity::CLIENT_CLASS,
            )
            .set("X-Kosmos-Client-Version", env!("CARGO_PKG_VERSION"))
            .set("X-Kosmos-Client-Pid", &std::process::id().to_string())
            .call()
            .map_err(|error| format!("no reply from the Engine: {error}"))?;
        let value: Value = response
            .into_json()
            .map_err(|error| format!("the Engine's reply is not json: {error}"))?;
        if value["ok"] != true {
            return Err(value["error"]
                .as_str()
                .unwrap_or("the Engine refused the call")
                .to_owned());
        }
        Ok(value)
    }

    /// POST /v1/rpc — every mutation and every list in the app goes through
    /// here, because the Engine owns the truth.
    pub fn rpc(&self, operation: &str, mut params: Value) -> Result<Value, String> {
        if !params.is_object() {
            return Err("local: params not an object".to_owned());
        }
        let lock = self.lock()?;
        params["operation"] = json!(operation);
        params["_req_id"] = json!(request_id());
        let response = self
            .agent()
            .post(&format!("http://127.0.0.1:{}/v1/rpc", lock.http_port))
            .set("Authorization", &format!("Bearer {}", lock.auth_token))
            .set("X-Kosmos-Api-Version", API_VERSION)
            .set(
                crate::identity::CLIENT_CLASS_HEADER,
                crate::identity::CLIENT_CLASS,
            )
            .set("X-Kosmos-Client-Version", env!("CARGO_PKG_VERSION"))
            .set("X-Kosmos-Client-Pid", &std::process::id().to_string())
            .send_json(params)
            .map_err(|error| format!("no reply from the Engine: {error}"))?;
        let value: Value = response
            .into_json()
            .map_err(|error| format!("the Engine's reply is not json: {error}"))?;
        if value["ok"] != true {
            return Err(value["error"]
                .as_str()
                .unwrap_or("the Engine refused the call")
                .to_owned());
        }
        // Mutations answer `{"ok":true,"data":…}`; callers want the data.
        Ok(value.get("data").cloned().unwrap_or(value))
    }

    fn agent(&self) -> ureq::Agent {
        ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(10))
            .build()
    }
}

/// A fresh id per call; the Engine uses it to tie a write to its log line.
fn request_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    format!("{now:x}-{:x}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Where the Engine keeps its lock file, most specific first. `MUNDUS_DATA_DIR`
/// wins so a dev Manager reads the dev Engine even when one is installed.
pub fn data_dir() -> Option<PathBuf> {
    data_dirs()
        .into_iter()
        .find(|dir| dir.join("engine.lock.json").is_file())
}

/// The dir the Engine is on, or the one it would use: the env override when it
/// names an existing dir, otherwise the roaming `Mundus`. The storage page shows
/// it before an Engine has ever written a lock.
pub fn data_home() -> Option<PathBuf> {
    let mut candidates = data_dirs();
    match candidates
        .iter()
        .position(|dir| dir.join("engine.lock.json").is_file())
    {
        Some(index) => Some(candidates.swap_remove(index)),
        None => candidates.into_iter().next(),
    }
    .or_else(|| config_root().map(|base| base.join("Mundus")))
}

/// Every dir the Engine may be running on, most specific first.
fn data_dirs() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = env_dir("MUNDUS_DATA_DIR") {
        candidates.push(dir);
    }
    // MIGRATION(KOS-267): the pre-rename variable still names a live Engine.
    if let Some(dir) = env_dir("KOSMOS_DATA_DIR") {
        candidates.push(dir);
    }
    if let Some(base) = config_root() {
        candidates.push(base.join("Mundus"));
        candidates.push(base.join("Kosmos"));
    }
    candidates
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|dir| dir.is_dir())
}

/// The roaming config root, exactly as `engine_base::data_dir` resolves it:
/// `%APPDATA%` on Windows, `~/Library/Application Support` on macOS (which does
/// not honour `XDG_*`), `$XDG_CONFIG_HOME` or `~/.config` on other Unix. A third
/// scheme here would send the Manager looking for a lock file the Engine never
/// wrote.
fn config_root() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        env_value("HOME").map(|home| home.join("Library").join("Application Support"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let appdata = env_value("APPDATA");
        if appdata.is_some() {
            return appdata;
        }
        #[cfg(unix)]
        {
            if let Some(xdg) = env_value("XDG_CONFIG_HOME") {
                return Some(xdg);
            }
            if let Some(home) = env_value("HOME") {
                return Some(home.join(".config"));
            }
        }
        None
    }
}

/// An environment directory, trimmed and refused when empty.
fn env_value(name: &str) -> Option<PathBuf> {
    let value = std::env::var(name).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// Polls the Engine for as long as the window lives, and pushes every change
/// into the pages' state. Runs on its own thread: a hung Engine must not hang
/// the window, and `ComponentContext` is made to be used from any thread.
pub fn watch(engine: Engine, context: ComponentContext, state: Arc<Mutex<Value>>) {
    std::thread::spawn(move || {
        let mut seen: Option<Instant> = None;
        let mut last: Option<(Value, Value, Value)> = None;
        loop {
            let engine_state = probe(&engine, &mut seen);
            // Pairing is app-wide: a request must reach the modal on whatever
            // page the user is on, so the snapshot is polled here and not with
            // the Devices page.
            let sync = engine
                .rpc("get_sync_snapshot", json!({}))
                .map(|snapshot| crate::data::sync_view(&snapshot))
                .unwrap_or_else(|_| {
                    (
                        json!({ "running": false, "incoming": [], "peers": [] }),
                        json!([]),
                    )
                });
            let (sync_state, peers) = sync;
            let reading = (engine_state, sync_state, peers);
            if last.as_ref() != Some(&reading) {
                last = Some(reading.clone());
                if let Ok(mut state) = state.lock() {
                    if let Some(object) = state.as_object_mut() {
                        object.insert("engine".into(), reading.0);
                        object.insert("sync".into(), reading.1);
                        object.insert("devices".into(), reading.2);
                    }
                    context.emit("state", state.clone());
                }
            }
            std::thread::sleep(POLL);
        }
    });
}

/// One reading of the Engine: whether it answers, and what it says it is.
fn probe(engine: &Engine, seen: &mut Option<Instant>) -> Value {
    let offline = json!({
        "status": "unreachable",
        "version": "",
        "channel": "production",
        "port": 0,
        "uptimeMinutes": 0,
    });
    let port = match engine.lock() {
        Ok(lock) => lock.http_port,
        Err(_) => return offline,
    };
    if engine.status("health").is_err() {
        *seen = None;
        return offline;
    }
    let started = *seen.get_or_insert_with(Instant::now);
    let info = engine.status("info").unwrap_or(Value::Null);
    let text = |key: &str, fallback: &str| {
        info.get(key)
            .and_then(Value::as_str)
            .unwrap_or(fallback)
            .to_owned()
    };
    json!({
        "status": "running",
        "version": text("version", ""),
        "channel": text("channel", "production"),
        "port": port,
        "uptimeMinutes": started.elapsed().as_secs() / 60,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_lock_reads_as_unreachable() {
        let engine = Engine::new(Some(PathBuf::from("C:/definitely/not/here")));
        let mut seen = None;
        let state = probe(&engine, &mut seen);
        assert_eq!(state["status"], "unreachable");
        assert!(seen.is_none());
    }

    /// Against a live Engine (`MUNDUS_DATA_DIR` set, Engine running):
    /// `cargo test --bin manager -- --ignored live_engine`.
    #[test]
    #[ignore = "needs a running Engine (`MUNDUS_DATA_DIR` set)"]
    fn live_engine_answers_status() {
        let engine = Engine::new(data_dir());
        let health = engine
            .status("health")
            .expect("the Engine answers /v1/health");
        assert_eq!(health["status"], "ready");
        let info = engine.status("info").expect("the Engine answers /v1/info");
        assert!(info["api_version"].is_string());
    }

    #[test]
    #[ignore = "needs a running Engine (`MUNDUS_DATA_DIR` set)"]
    fn live_engine_answers_a_list_call() {
        let engine = Engine::new(data_dir());
        let listed = engine
            .rpc("app_index.list_all", json!({}))
            .expect("app_index.list_all succeeds");
        let apps = listed["apps"]
            .as_array()
            .expect("a list of launchable apps");
        let id = apps
            .first()
            .and_then(|app| app["id"].as_str())
            .expect("the index names its rows");
        // `app_index.icon_path` looks the id up the same way `app_index.launch`
        // does, so this answers whether launching that exact id would resolve.
        let icon = engine
            .rpc("app_index.icon_path", json!({ "id": id }))
            .expect("an id from list_all resolves in the index");
        assert!(icon.get("path").is_some() || icon.is_object());
    }

    #[test]
    fn request_ids_do_not_repeat() {
        let ids: Vec<_> = (0..64).map(|_| request_id()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
    }
}
