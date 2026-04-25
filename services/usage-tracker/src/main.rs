#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
mod windows_capture;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use ark_core::db::{
    bump_sync_version_vector, get_sync_kv, init_schema, open_db, set_sync_kv, upsert_tracked_app,
    upsert_usage_event, upsert_usage_session,
};
use ark_core::types::{TrackedApp, UsageEvent, UsageSession};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[cfg(target_os = "windows")]
use windows_capture::capture_foreground_window;
#[cfg(target_os = "windows")]
use windows_capture::ForegroundWindowSample;

const PLATFORM: &str = "windows";
const TRACKER_DEVICE_ID_KEY: &str = "usage_tracker.device_id";

#[derive(Debug, Clone)]
struct Config {
    ark_db_path: PathBuf,
    poll_interval: Duration,
    idle_threshold: Duration,
    run_once: bool,
}

#[derive(Debug, Clone)]
struct TrackerIdentity {
    device_id: String,
    device_name: String,
}

#[derive(Debug, Clone)]
struct ActiveSession {
    tracked_app: TrackedApp,
    session_id: String,
    started_at: String,
    foreground_ms: i64,
    idle_ms: i64,
    window_title: Option<String>,
    process_name: String,
    exe_path: String,
    pid_start: i64,
    pid_end: i64,
    current_is_idle: bool,
    sample_count: u64,
}

impl ActiveSession {
    fn matches(&self, sample: &ForegroundWindowSample) -> bool {
        self.tracked_app.id == sample.tracked_app_id && self.pid_end == i64::from(sample.pid)
    }

    fn accumulate(&mut self, delta_ms: i64) {
        if self.current_is_idle {
            self.idle_ms += delta_ms;
        } else {
            self.foreground_ms += delta_ms;
        }
        self.sample_count += 1;
    }

    fn to_usage_session(
        &self,
        identity: &TrackerIdentity,
        ended_at: Option<String>,
    ) -> UsageSession {
        UsageSession {
            id: self.session_id.clone(),
            tracked_app_id: self.tracked_app.id.clone(),
            device_id: identity.device_id.clone(),
            device_name: identity.device_name.clone(),
            platform: PLATFORM.to_string(),
            started_at: self.started_at.clone(),
            ended_at,
            foreground_ms: self.foreground_ms,
            idle_ms: self.idle_ms,
            window_title: self.window_title.clone(),
            process_name: self.process_name.clone(),
            exe_path: self.exe_path.clone(),
            pid_start: Some(self.pid_start),
            pid_end: Some(self.pid_end),
            meta_json: json!({
                "source": "usage-tracker",
                "sampleCount": self.sample_count,
            }),
        }
    }
}

fn main() {
    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("usage-tracker currently supports Windows only.");
        std::process::exit(1);
    }

    #[cfg(target_os = "windows")]
    {
        if let Err(error) = run() {
            eprintln!("usage-tracker fatal: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(target_os = "windows")]
fn run() -> Result<(), String> {
    let config = Config::from_env_and_args()?;
    ensure_parent_dir(&config.ark_db_path)?;

    let conn = open_db(
        config
            .ark_db_path
            .to_str()
            .ok_or_else(|| "Invalid Ark DB path".to_string())?,
    )?;
    init_schema(&conn)?;

    let identity = TrackerIdentity {
        device_id: load_or_create_tracker_device_id(&conn)?,
        device_name: resolve_device_name(),
    };

    let mut current_session: Option<ActiveSession> = None;
    let mut previous_tick = Instant::now();

    loop {
        let sample = capture_foreground_window(config.idle_threshold)?;
        let captured_at = iso_now();
        let delta_ms = previous_tick.elapsed().as_millis().min(i64::MAX as u128) as i64;

        if let Some(session) = current_session.as_mut() {
            session.accumulate(delta_ms);
            persist_usage_session(
                &conn,
                &session.to_usage_session(&identity, None),
                &identity.device_id,
            )?;
        }

        reconcile_session(
            &conn,
            &identity,
            &mut current_session,
            sample,
            captured_at,
            config.poll_interval.as_millis() as i64,
        )?;

        if config.run_once {
            if let Some(session) = current_session.as_ref() {
                persist_usage_session(
                    &conn,
                    &session.to_usage_session(&identity, None),
                    &identity.device_id,
                )?;
            }
            return Ok(());
        }

        previous_tick = Instant::now();
        thread::sleep(config.poll_interval);
    }
}

#[cfg(target_os = "windows")]
fn reconcile_session(
    conn: &Connection,
    identity: &TrackerIdentity,
    current_session: &mut Option<ActiveSession>,
    sample: Option<ForegroundWindowSample>,
    captured_at: String,
    poll_interval_ms: i64,
) -> Result<(), String> {
    match (current_session.as_mut(), sample) {
        (Some(active), Some(sample)) if active.matches(&sample) => {
            let previous_window_title = active.window_title.clone();
            let previous_idle = active.current_is_idle;
            active.window_title = sample.window_title.clone();
            active.current_is_idle = sample.is_idle;
            active.pid_end = i64::from(sample.pid);
            if previous_window_title != sample.window_title {
                persist_usage_event(
                    conn,
                    &build_event(
                        active,
                        identity,
                        "window_changed",
                        captured_at.clone(),
                        json!({
                            "pollIntervalMs": poll_interval_ms,
                            "windowTitle": sample.window_title,
                        }),
                    ),
                    &identity.device_id,
                )?;
            }
            if previous_idle != sample.is_idle {
                persist_usage_event(
                    conn,
                    &build_event(
                        active,
                        identity,
                        if sample.is_idle {
                            "idle_started"
                        } else {
                            "idle_ended"
                        },
                        captured_at,
                        json!({
                            "idle": sample.is_idle,
                            "pollIntervalMs": poll_interval_ms,
                        }),
                    ),
                    &identity.device_id,
                )?;
            }
            Ok(())
        }
        (Some(active), Some(sample)) => {
            finalize_session(conn, identity, active, captured_at.clone(), "session_ended")?;
            let new_session = start_session(conn, identity, sample, captured_at, poll_interval_ms)?;
            *current_session = Some(new_session);
            Ok(())
        }
        (Some(active), None) => {
            finalize_session(conn, identity, active, captured_at, "session_ended")?;
            *current_session = None;
            Ok(())
        }
        (None, Some(sample)) => {
            let new_session = start_session(conn, identity, sample, captured_at, poll_interval_ms)?;
            *current_session = Some(new_session);
            Ok(())
        }
        (None, None) => Ok(()),
    }
}

#[cfg(target_os = "windows")]
fn start_session(
    conn: &Connection,
    identity: &TrackerIdentity,
    sample: ForegroundWindowSample,
    captured_at: String,
    poll_interval_ms: i64,
) -> Result<ActiveSession, String> {
    let first_seen_at = load_existing_first_seen(conn, &sample.tracked_app_id)?
        .unwrap_or_else(|| captured_at.clone());
    let tracked_app = TrackedApp {
        id: sample.tracked_app_id.clone(),
        platform: PLATFORM.to_string(),
        exe_path: sample.exe_path.clone(),
        normalized_exe_path: sample.normalized_exe_path.clone(),
        process_name: sample.process_name.clone(),
        display_name: derive_display_name(&sample.process_name, &sample.window_title),
        publisher: None,
        icon_ref: None,
        first_seen_at,
        last_seen_at: captured_at.clone(),
    };

    persist_tracked_app(conn, &tracked_app, &identity.device_id)?;

    let session = ActiveSession {
        tracked_app,
        session_id: Uuid::new_v4().to_string(),
        started_at: captured_at.clone(),
        foreground_ms: 0,
        idle_ms: 0,
        window_title: sample.window_title.clone(),
        process_name: sample.process_name.clone(),
        exe_path: sample.exe_path.clone(),
        pid_start: i64::from(sample.pid),
        pid_end: i64::from(sample.pid),
        current_is_idle: sample.is_idle,
        sample_count: 1,
    };

    persist_usage_session(
        conn,
        &session.to_usage_session(identity, None),
        &identity.device_id,
    )?;
    persist_usage_event(
        conn,
        &build_event(
            &session,
            identity,
            "session_started",
            captured_at,
            json!({
                "idle": sample.is_idle,
                "pollIntervalMs": poll_interval_ms,
            }),
        ),
        &identity.device_id,
    )?;

    Ok(session)
}

#[cfg(target_os = "windows")]
fn finalize_session(
    conn: &Connection,
    identity: &TrackerIdentity,
    active: &ActiveSession,
    ended_at: String,
    kind: &str,
) -> Result<(), String> {
    persist_usage_session(
        conn,
        &active.to_usage_session(identity, Some(ended_at.clone())),
        &identity.device_id,
    )?;
    persist_usage_event(
        conn,
        &build_event(
            active,
            identity,
            kind,
            ended_at,
            json!({
                "foregroundMs": active.foreground_ms,
                "idleMs": active.idle_ms,
                "sampleCount": active.sample_count,
            }),
        ),
        &identity.device_id,
    )?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn build_event(
    active: &ActiveSession,
    identity: &TrackerIdentity,
    kind: &str,
    occurred_at: String,
    meta_json: Value,
) -> UsageEvent {
    UsageEvent {
        id: Uuid::new_v4().to_string(),
        tracked_app_id: active.tracked_app.id.clone(),
        usage_session_id: Some(active.session_id.clone()),
        device_id: identity.device_id.clone(),
        device_name: identity.device_name.clone(),
        platform: PLATFORM.to_string(),
        occurred_at,
        kind: kind.to_string(),
        window_title: active.window_title.clone(),
        process_name: active.process_name.clone(),
        exe_path: active.exe_path.clone(),
        pid: Some(active.pid_end),
        is_foreground: true,
        is_idle: active.current_is_idle,
        meta_json,
    }
}

fn load_or_create_tracker_device_id(conn: &Connection) -> Result<String, String> {
    if let Some(existing) = get_sync_kv(conn, TRACKER_DEVICE_ID_KEY)? {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let generated = format!("usage-tracker-{}", Uuid::new_v4().simple());
    set_sync_kv(conn, TRACKER_DEVICE_ID_KEY, &generated)?;
    Ok(generated)
}

fn load_existing_first_seen(
    conn: &Connection,
    tracked_app_id: &str,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT first_seen_at FROM tracked_apps WHERE id = ?1",
        [tracked_app_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn resolve_device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Windows Device".to_string())
}

fn persist_tracked_app(
    conn: &Connection,
    tracked_app: &TrackedApp,
    device_id: &str,
) -> Result<(), String> {
    upsert_tracked_app(conn, tracked_app)?;
    bump_sync_version_vector(conn, &tracked_app.id, device_id).map(|_| ())
}

fn persist_usage_session(
    conn: &Connection,
    session: &UsageSession,
    device_id: &str,
) -> Result<(), String> {
    upsert_usage_session(conn, session)?;
    bump_sync_version_vector(conn, &session.id, device_id).map(|_| ())
}

fn persist_usage_event(
    conn: &Connection,
    event: &UsageEvent,
    device_id: &str,
) -> Result<(), String> {
    upsert_usage_event(conn, event)?;
    bump_sync_version_vector(conn, &event.id, device_id).map(|_| ())
}

impl Config {
    fn from_env_and_args() -> Result<Self, String> {
        let mut config = Self {
            ark_db_path: default_ark_db_path()?,
            poll_interval: Duration::from_millis(read_u64_env("USAGE_TRACKER_POLL_MS", 5_000)),
            idle_threshold: Duration::from_secs(read_u64_env("USAGE_TRACKER_IDLE_SECS", 60)),
            run_once: env::var("USAGE_TRACKER_RUN_ONCE")
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        };

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--db-path" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--db-path requires a value".to_string())?;
                    config.ark_db_path = PathBuf::from(value);
                }
                "--poll-ms" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--poll-ms requires a value".to_string())?;
                    config.poll_interval =
                        Duration::from_millis(parse_u64_arg("--poll-ms", &value)?);
                }
                "--idle-secs" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--idle-secs requires a value".to_string())?;
                    config.idle_threshold =
                        Duration::from_secs(parse_u64_arg("--idle-secs", &value)?);
                }
                "--once" => {
                    config.run_once = true;
                }
                other => {
                    return Err(format!("Unknown argument: {other}"));
                }
            }
        }

        Ok(config)
    }
}

fn default_ark_db_path() -> Result<PathBuf, String> {
    if let Ok(path_override) = env::var("ARK_DB_PATH") {
        if !path_override.trim().is_empty() {
            return Ok(PathBuf::from(path_override));
        }
    }

    let app_data = env::var("APPDATA")
        .map(PathBuf::from)
        .or_else(|_| env::var("LOCALAPPDATA").map(PathBuf::from))
        .map_err(|_| "APPDATA or LOCALAPPDATA is required to resolve Ark DB path".to_string())?;

    Ok(app_data.join("Kepler").join("ark.db"))
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn read_u64_env(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn parse_u64_arg(flag: &str, value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{flag} expects a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{flag} expects a positive integer"));
    }
    Ok(parsed)
}

fn normalize_exe_path(input: &str) -> String {
    input.trim().replace('/', "\\").to_ascii_lowercase()
}

fn derive_display_name(process_name: &str, window_title: &Option<String>) -> Option<String> {
    if let Some(title) = window_title
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return Some(title.to_string());
    }

    Path::new(process_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
}

fn tracked_app_id_for(exe_path: &str) -> String {
    let normalized = normalize_exe_path(exe_path);
    let mut hasher = Sha256::new();
    hasher.update(format!("{PLATFORM}:{normalized}").as_bytes());
    format!("{:x}", hasher.finalize())
}

fn iso_now() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_windows_paths() {
        assert_eq!(
            normalize_exe_path("C:/Games/Demo/Game.EXE"),
            "c:\\games\\demo\\game.exe"
        );
    }

    #[test]
    fn tracked_app_id_is_stable() {
        let first = tracked_app_id_for("C:/Games/Demo/Game.EXE");
        let second = tracked_app_id_for("c:\\games\\demo\\game.exe");
        assert_eq!(first, second);
    }

    #[test]
    fn persist_tracked_app_records_sync_state_with_ark_core_helper() {
        let conn = open_db(":memory:").unwrap();
        init_schema(&conn).unwrap();
        let tracked_app = TrackedApp {
            id: "app-local".to_string(),
            platform: PLATFORM.to_string(),
            exe_path: "C:\\Games\\Demo\\demo.exe".to_string(),
            normalized_exe_path: "c:\\games\\demo\\demo.exe".to_string(),
            process_name: "demo.exe".to_string(),
            display_name: Some("Demo".to_string()),
            publisher: None,
            icon_ref: None,
            first_seen_at: "2026-04-24T00:00:00.000Z".to_string(),
            last_seen_at: "2026-04-24T00:00:00.000Z".to_string(),
        };

        persist_tracked_app(&conn, &tracked_app, "usage-device").unwrap();

        let raw = get_sync_kv(&conn, "lan_sync.version_vector")
            .unwrap()
            .expect("version vector should be stored");
        let vector: ark_core::types::VersionVector = serde_json::from_str(&raw).unwrap();
        assert!(
            vector
                .get("app-local")
                .is_some_and(|hlc| hlc.ends_with(":usage-device")),
            "tracked app should have a usage-device HLC"
        );
    }

    #[test]
    fn tracker_device_id_is_stored_in_sync_kv() {
        let conn = open_db(":memory:").unwrap();
        init_schema(&conn).unwrap();

        let generated = load_or_create_tracker_device_id(&conn).unwrap();
        assert!(generated.starts_with("usage-tracker-"));
        assert_eq!(
            get_sync_kv(&conn, TRACKER_DEVICE_ID_KEY).unwrap(),
            Some(generated.clone())
        );
        assert_eq!(load_or_create_tracker_device_id(&conn).unwrap(), generated);
    }
}
