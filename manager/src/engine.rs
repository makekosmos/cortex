//! The Engine's HTTP surface: lock discovery, status reads and `/v1/rpc`.
//!
//! The pages never touch it. They send a command to the bridge; this side calls
//! the Engine and pushes the resulting state back down. Discovery reads
//! `engine.lock.json` on every call, so an Engine restarted under a new port is
//! picked up on the next poll without the Manager noticing.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
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
    pick_data_home(data_dirs(), config_root())
}

/// The choice `data_home` makes, split out from the environment so it can be
/// tested: the dir holding a lock wins, otherwise the most specific candidate.
fn pick_data_home(mut candidates: Vec<PathBuf>, root: Option<PathBuf>) -> Option<PathBuf> {
    match candidates
        .iter()
        .position(|dir| dir.join("engine.lock.json").is_file())
    {
        Some(index) => Some(candidates.swap_remove(index)),
        None => candidates.into_iter().next(),
    }
    .or_else(|| root.map(|base| base.join("Mundus")))
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
        let mut last: Option<(Value, Value)> = None;
        loop {
            let engine_state = probe(&engine);
            // Pairing is app-wide: a request must reach the modal on whatever
            // page the user is on, so the snapshot is polled here and not with
            // the Devices page. A failed read keeps the previous answer: the
            // Engine still holds the request, and an empty list here would close
            // a modal the user has not answered.
            let sync = engine
                .rpc("get_sync_snapshot", json!({}))
                .map(|snapshot| crate::data::sync_view(&snapshot))
                .or_else(|message| {
                    tracing::warn!(error = %message, "sync snapshot failed");
                    last.as_ref().map(|(_, sync)| sync.clone()).ok_or(message)
                })
                .unwrap_or_else(|_| no_sync());
            let reading = (engine_state, sync);
            if last.as_ref() != Some(&reading) {
                last = Some(reading.clone());
                if let Ok(mut state) = state.lock() {
                    let shown = state.get("sync").cloned();
                    if let Some(object) = state.as_object_mut() {
                        object.insert("engine".into(), reading.0);
                        object.insert("sync".into(), with_shown_code(reading.1, shown.as_ref()));
                    }
                    context.emit("state", state.clone());
                }
            }
            std::thread::sleep(POLL);
        }
    });
}

/// What the pages show while no snapshot has arrived.
fn no_sync() -> Value {
    json!({ "running": false, "incoming": [], "peers": [] })
}

/// The snapshot replaces `sync`, except for the pairing code the user asked to
/// see: that one is the Manager's own answer, and the poll two seconds later
/// must not blank a code the user is reading out.
fn with_shown_code(mut sync: Value, previous: Option<&Value>) -> Value {
    let shown = previous
        .and_then(|sync| sync.get("ownCode"))
        .cloned()
        .unwrap_or(Value::Null);
    if let Some(object) = sync.as_object_mut() {
        object.insert("ownCode".into(), shown);
    }
    sync
}

/// One reading of the Engine: whether it answers, and what it says it is.
fn probe(engine: &Engine) -> Value {
    let offline = json!({
        "status": "unreachable",
        "version": "",
        "channel": "production",
        "port": 0,
    });
    let port = match engine.lock() {
        Ok(lock) => lock.http_port,
        Err(_) => return offline,
    };
    if engine.status("health").is_err() {
        return offline;
    }
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_lock_reads_as_unreachable() {
        let engine = Engine::new(Some(PathBuf::from("C:/definitely/not/here")));
        let state = probe(&engine);
        assert_eq!(state["status"], "unreachable");
        assert_eq!(state["port"], 0);
    }

    /// The pairing code the user asked to see outlives the poll that follows it:
    /// the snapshot carries no code, so a plain replace would blank the row the
    /// user is reading out loud.
    #[test]
    fn the_shown_pairing_code_survives_a_poll() {
        let shown = json!({ "running": true, "incoming": [], "peers": [], "ownCode": "123456" });
        let next = json!({ "running": true, "incoming": [], "peers": [] });
        assert_eq!(
            with_shown_code(next.clone(), Some(&shown))["ownCode"],
            "123456"
        );
        assert_eq!(with_shown_code(next.clone(), None)["ownCode"], Value::Null);
        // Hiding it is the Manager's own answer, and it sticks.
        let hidden = json!({ "running": true, "incoming": [], "peers": [], "ownCode": "" });
        assert_eq!(with_shown_code(next, Some(&hidden))["ownCode"], "");
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
    fn live_engine_answers_the_settings_calls_the_page_makes() {
        let engine = Engine::new(data_dir());
        let settings = engine
            .rpc("engine.settings.get", json!({}))
            .expect("engine.settings.get succeeds");
        assert!(settings.is_object(), "settings is an object");

        // `{available, enabled, reason}` is the object the state must narrow to
        // the switch's position — the object itself is truthy in JavaScript.
        let autostart = engine
            .rpc("engine.autostart.get", json!({}))
            .expect("engine.autostart.get succeeds");
        let enabled = autostart["enabled"]
            .as_bool()
            .expect("the Engine states the switch's position");
        let mut state = json!({});
        crate::data::apply(&mut state, "settings.autostart", autostart, None);
        assert_eq!(state["settings"]["autostart"], json!(enabled));
    }

    /// `Stop` deletes `engine.lock.json`; the directory must still be named,
    /// or `Start` has nothing to hand the Engine and refuses to run without
    /// `MUNDUS_DATA_DIR`. No temp dir is created: the paths simply have no lock.
    #[test]
    fn a_stopped_engine_still_names_its_data_directory() {
        let stopped = PathBuf::from("nowhere/mundus-dev");
        let legacy = PathBuf::from("nowhere/legacy-dir");
        assert_eq!(
            pick_data_home(vec![stopped.clone(), legacy], None),
            Some(stopped)
        );

        // The roaming dir is the last resort, as in `engine_base::data_dir`.
        let root = PathBuf::from("nowhere/roaming");
        assert_eq!(
            pick_data_home(Vec::new(), Some(root.clone())),
            Some(root.join("Mundus"))
        );
        assert_eq!(pick_data_home(Vec::new(), None), None);
    }

    #[test]
    fn request_ids_do_not_repeat() {
        let ids: Vec<_> = (0..64).map(|_| request_id()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
    }
}
