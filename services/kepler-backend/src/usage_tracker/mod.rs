// Usage tracker module — портированно из services/usage-tracker (Phase E1
// refactor). Запускается как tokio task внутри kepler-backend, пишет
// foreground-session-данные напрямую в ARK через ArkHost (in-process), без WS
// round-trip и без отдельного singleton-процесса.
//
// Что НЕ переехало (по сравнению со standalone usage-tracker):
//   * singleton lock — backend сам singleton (через SingletonGuard в main.rs),
//   * HKCU\Run installer — это shell installer ответственность,
//   * kepler_client.rs (WS) — мы внутри backend'а, прямой вызов ArkHost.
//   * spool — больше не нужен: нет WS round-trip и нет race с cold-start.

#[cfg(target_os = "windows")]
mod windows_capture;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::task::JoinHandle;

use crate::ark_host::ArkHost;

#[cfg(target_os = "windows")]
use windows_capture::{capture_foreground_window, ForegroundWindowSample, PLATFORM};

const TRACKER_DEVICE_ID_KEY: &str = "usage_tracker.device_id";
const DEFAULT_POLL_MS: u64 = 5_000;
const DEFAULT_IDLE_SECS: u64 = 60;

/// Дефолтный blocklist для privacy. Match — case-insensitive substring в
/// process name или window title. Если sample матчится — он не пишется в БД
/// (ни tracked_app, ни session, ни event). Юзер не увидит password manager'ы
/// в Dashboard.
const DEFAULT_EXCLUDE_PATTERNS: &[&str] = &[
    "1password",
    "keepass",
    "bitwarden",
    "lastpass",
    "password",
    "credential",
];

/// Конфиг tracker'а. `from_env()` читает те же ENV-ключи, что и старый standalone
/// usage-tracker, чтобы операционные привычки и smoke-тесты не сломались.
#[derive(Debug, Clone)]
pub struct UsageTrackerOpts {
    pub poll_interval: Duration,
    pub idle_threshold: Duration,
    /// Lowercased substrings; sample матчится если ЛЮБОЙ из паттернов входит
    /// в process_name ИЛИ в window_title. Дефолт — DEFAULT_EXCLUDE_PATTERNS.
    pub exclude_patterns: Vec<String>,
}

impl Default for UsageTrackerOpts {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(DEFAULT_POLL_MS),
            idle_threshold: Duration::from_secs(DEFAULT_IDLE_SECS),
            exclude_patterns: DEFAULT_EXCLUDE_PATTERNS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        }
    }
}

impl UsageTrackerOpts {
    pub fn from_env() -> Self {
        let poll_ms = std::env::var("USAGE_TRACKER_POLL_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_POLL_MS);
        let idle_secs = std::env::var("USAGE_TRACKER_IDLE_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_IDLE_SECS);
        // KEPLER_USAGE_TRACKER_EXCLUDE_EXTRA — comma-separated user добавки
        // поверх дефолтного списка. Пустая строка / отсутствие = только дефолт.
        let mut excludes: Vec<String> = DEFAULT_EXCLUDE_PATTERNS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        if let Ok(extra) = std::env::var("KEPLER_USAGE_TRACKER_EXCLUDE_EXTRA") {
            for token in extra.split(',') {
                let trimmed = token.trim().to_lowercase();
                if !trimmed.is_empty() && !excludes.contains(&trimmed) {
                    excludes.push(trimmed);
                }
            }
        }
        Self {
            poll_interval: Duration::from_millis(poll_ms),
            idle_threshold: Duration::from_secs(idle_secs),
            exclude_patterns: excludes,
        }
    }

    fn matches_exclude(&self, process_name: &str, window_title: Option<&str>) -> bool {
        let proc_lower = process_name.to_lowercase();
        let title_lower = window_title.map(|t| t.to_lowercase());
        for pat in &self.exclude_patterns {
            if proc_lower.contains(pat) {
                return true;
            }
            if let Some(t) = &title_lower {
                if t.contains(pat) {
                    return true;
                }
            }
        }
        false
    }
}

/// Запустить usage-tracker в фоне. Возвращает JoinHandle, который никогда не
/// resolve'ится при штатной работе (loop бесконечный). Caller может drop'нуть
/// handle если хочет fire-and-forget.
#[cfg(target_os = "windows")]
pub fn spawn(ark: Arc<ArkHost>, opts: UsageTrackerOpts) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = run(ark, opts).await {
            eprintln!("[usage-tracker] FATAL: {e}");
        }
    })
}

#[cfg(not(target_os = "windows"))]
pub fn spawn(_ark: Arc<ArkHost>, _opts: UsageTrackerOpts) -> JoinHandle<()> {
    tokio::spawn(async move {
        eprintln!("[usage-tracker] disabled on non-Windows platforms");
    })
}

#[cfg(target_os = "windows")]
async fn run(ark: Arc<ArkHost>, opts: UsageTrackerOpts) -> Result<(), String> {
    let identity = TrackerIdentity {
        device_id: load_or_create_tracker_device_id(&ark).await?,
        device_name: resolve_device_name(),
    };
    eprintln!(
        "[usage-tracker] started: device_id={} device_name={:?}",
        identity.device_id, identity.device_name
    );

    let mut current_session: Option<ActiveSession> = None;
    let mut previous_tick = std::time::Instant::now();
    // first_seen_at cache: stable между запусками backend'а сохраняется через
    // upsert (DB) — но в рамках одного процесса нам достаточно in-memory HashMap,
    // потому что мы первый раз увидим приложение в этой сессии и зафиксируем
    // captured_at; последующие session_started для того же приложения возьмут
    // ту же дату. Между перезапусками backend'а first_seen_at будет «сегодня» —
    // приемлемо, regression от прежнего поведения минимальная (см. proposal 8.4).
    let mut first_seen_cache: HashMap<String, String> = HashMap::new();

    loop {
        let idle = opts.idle_threshold;
        let raw_sample = tokio::task::spawn_blocking(move || capture_foreground_window(idle))
            .await
            .map_err(|e| format!("capture join: {e}"))??;
        // Privacy filter — password manager'ы и подобные не пишем в БД.
        // Treat'им как «нет foreground окна» → текущая сессия finalize'нется,
        // новая не стартует пока exclude активен.
        let sample = raw_sample.filter(|s| {
            !opts.matches_exclude(&s.process_name, s.window_title.as_deref())
        });
        let captured_at = iso_now();
        let delta_ms = previous_tick.elapsed().as_millis().min(i64::MAX as u128) as i64;

        if let Some(session) = current_session.as_mut() {
            session.accumulate(delta_ms);
            persist_usage_session(
                &ark,
                &session.to_usage_session(&identity, None),
                &identity.device_id,
            )
            .await?;
        }

        reconcile_session(
            &ark,
            &identity,
            &mut current_session,
            &mut first_seen_cache,
            sample,
            captured_at,
            opts.poll_interval.as_millis() as i64,
        )
        .await?;

        previous_tick = std::time::Instant::now();
        tokio::time::sleep(opts.poll_interval).await;
    }
}

#[derive(Debug, Clone)]
struct TrackerIdentity {
    device_id: String,
    device_name: String,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct ActiveSession {
    tracked_app: TrackedAppFields,
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

/// Локальный snapshot полей TrackedApp. Не используем ark_core::types::TrackedApp
/// напрямую чтобы не плодить дублирующий type-import — JSON envelope build'ится
/// руками (тот же shape что у RPC operation `upsert_tracked_app`).
#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct TrackedAppFields {
    id: String,
    platform: String,
    exe_path: String,
    normalized_exe_path: String,
    process_name: String,
    display_name: Option<String>,
    first_seen_at: String,
    last_seen_at: String,
}

#[cfg(target_os = "windows")]
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

    fn to_usage_session(&self, identity: &TrackerIdentity, ended_at: Option<String>) -> Value {
        json!({
            "id": self.session_id,
            "trackedAppId": self.tracked_app.id,
            "deviceId": identity.device_id,
            "deviceName": identity.device_name,
            "platform": PLATFORM,
            "startedAt": self.started_at,
            "endedAt": ended_at,
            "foregroundMs": self.foreground_ms,
            "idleMs": self.idle_ms,
            "windowTitle": self.window_title,
            "processName": self.process_name,
            "exePath": self.exe_path,
            "pidStart": self.pid_start,
            "pidEnd": self.pid_end,
            "metaJson": {
                "source": "usage-tracker",
                "sampleCount": self.sample_count,
            },
        })
    }
}

#[cfg(target_os = "windows")]
async fn reconcile_session(
    ark: &Arc<ArkHost>,
    identity: &TrackerIdentity,
    current_session: &mut Option<ActiveSession>,
    first_seen_cache: &mut HashMap<String, String>,
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
                    ark,
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
                )
                .await?;
            }
            if previous_idle != sample.is_idle {
                persist_usage_event(
                    ark,
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
                )
                .await?;
            }
            Ok(())
        }
        (Some(active), Some(sample)) => {
            finalize_session(ark, identity, active, captured_at.clone(), "session_ended").await?;
            let new_session = start_session(
                ark,
                identity,
                first_seen_cache,
                sample,
                captured_at,
                poll_interval_ms,
            )
            .await?;
            *current_session = Some(new_session);
            Ok(())
        }
        (Some(active), None) => {
            finalize_session(ark, identity, active, captured_at, "session_ended").await?;
            *current_session = None;
            Ok(())
        }
        (None, Some(sample)) => {
            let new_session = start_session(
                ark,
                identity,
                first_seen_cache,
                sample,
                captured_at,
                poll_interval_ms,
            )
            .await?;
            *current_session = Some(new_session);
            Ok(())
        }
        (None, None) => Ok(()),
    }
}

#[cfg(target_os = "windows")]
async fn start_session(
    ark: &Arc<ArkHost>,
    identity: &TrackerIdentity,
    first_seen_cache: &mut HashMap<String, String>,
    sample: ForegroundWindowSample,
    captured_at: String,
    poll_interval_ms: i64,
) -> Result<ActiveSession, String> {
    let first_seen_at = first_seen_cache
        .entry(sample.tracked_app_id.clone())
        .or_insert_with(|| captured_at.clone())
        .clone();
    let tracked_app = TrackedAppFields {
        id: sample.tracked_app_id.clone(),
        platform: PLATFORM.to_string(),
        exe_path: sample.exe_path.clone(),
        normalized_exe_path: sample.normalized_exe_path.clone(),
        process_name: sample.process_name.clone(),
        display_name: derive_display_name(&sample.process_name, &sample.window_title),
        first_seen_at,
        last_seen_at: captured_at.clone(),
    };

    persist_tracked_app(ark, &tracked_app, &identity.device_id).await?;

    let session = ActiveSession {
        tracked_app,
        session_id: new_uuid(),
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
        ark,
        &session.to_usage_session(identity, None),
        &identity.device_id,
    )
    .await?;
    persist_usage_event(
        ark,
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
    )
    .await?;

    Ok(session)
}

#[cfg(target_os = "windows")]
async fn finalize_session(
    ark: &Arc<ArkHost>,
    identity: &TrackerIdentity,
    active: &ActiveSession,
    ended_at: String,
    kind: &str,
) -> Result<(), String> {
    persist_usage_session(
        ark,
        &active.to_usage_session(identity, Some(ended_at.clone())),
        &identity.device_id,
    )
    .await?;
    persist_usage_event(
        ark,
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
    )
    .await?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn build_event(
    active: &ActiveSession,
    identity: &TrackerIdentity,
    kind: &str,
    occurred_at: String,
    meta_json: Value,
) -> Value {
    json!({
        "id": new_uuid(),
        "trackedAppId": active.tracked_app.id,
        "usageSessionId": active.session_id,
        "deviceId": identity.device_id,
        "deviceName": identity.device_name,
        "platform": PLATFORM,
        "occurredAt": occurred_at,
        "kind": kind,
        "windowTitle": active.window_title,
        "processName": active.process_name,
        "exePath": active.exe_path,
        "pid": active.pid_end,
        "isForeground": true,
        "isIdle": active.current_is_idle,
        "metaJson": meta_json,
    })
}

// ----- ARK access helpers (через ArkHost) -----

async fn ark_request(ark: &Arc<ArkHost>, op: &str, params: Value) -> Result<Value, String> {
    let resp = ark
        .request(op, params)
        .await
        .map_err(|e| format!("ark_host {op}: {e}"))?;
    if !resp.ok {
        return Err(format!(
            "ark_host {op}: {}",
            resp.error.unwrap_or_else(|| "unknown error".to_string())
        ));
    }
    Ok(resp.data)
}

async fn load_or_create_tracker_device_id(ark: &Arc<ArkHost>) -> Result<String, String> {
    let existing = ark_request(
        ark,
        "get_sync_kv",
        json!({ "key": TRACKER_DEVICE_ID_KEY }),
    )
    .await?;
    if let Some(value) = existing.as_str() {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let generated = format!("usage-tracker-{}", new_uuid_simple());
    ark_request(
        ark,
        "set_sync_kv",
        json!({ "key": TRACKER_DEVICE_ID_KEY, "value": generated }),
    )
    .await?;
    Ok(generated)
}

#[cfg(target_os = "windows")]
async fn persist_tracked_app(
    ark: &Arc<ArkHost>,
    tracked_app: &TrackedAppFields,
    device_id: &str,
) -> Result<(), String> {
    let payload = json!({
        "id": tracked_app.id,
        "platform": tracked_app.platform,
        "exePath": tracked_app.exe_path,
        "normalizedExePath": tracked_app.normalized_exe_path,
        "processName": tracked_app.process_name,
        "displayName": tracked_app.display_name,
        "firstSeenAt": tracked_app.first_seen_at,
        "lastSeenAt": tracked_app.last_seen_at,
    });
    ark_request(
        ark,
        "upsert_tracked_app",
        json!({ "tracked_app": payload, "device_id": device_id }),
    )
    .await
    .map(|_| ())
}

async fn persist_usage_session(
    ark: &Arc<ArkHost>,
    session: &Value,
    device_id: &str,
) -> Result<(), String> {
    ark_request(
        ark,
        "upsert_usage_session",
        json!({ "usage_session": session, "device_id": device_id }),
    )
    .await
    .map(|_| ())
}

async fn persist_usage_event(
    ark: &Arc<ArkHost>,
    event: &Value,
    device_id: &str,
) -> Result<(), String> {
    ark_request(
        ark,
        "upsert_usage_event",
        json!({ "usage_event": event, "device_id": device_id }),
    )
    .await
    .map(|_| ())
}

// ----- utils -----

fn resolve_device_name() -> String {
    std::env::var("KOSMOS_DEVICE_NAME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "Windows Device".to_string())
}

#[cfg(target_os = "windows")]
fn derive_display_name(process_name: &str, window_title: &Option<String>) -> Option<String> {
    if let Some(title) = window_title
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return Some(title.to_string());
    }
    std::path::Path::new(process_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
}

fn iso_now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn new_uuid_simple() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opts_from_env_uses_defaults_when_unset() {
        std::env::remove_var("USAGE_TRACKER_POLL_MS");
        std::env::remove_var("USAGE_TRACKER_IDLE_SECS");
        std::env::remove_var("KEPLER_USAGE_TRACKER_EXCLUDE_EXTRA");
        let opts = UsageTrackerOpts::from_env();
        assert_eq!(opts.poll_interval, Duration::from_millis(DEFAULT_POLL_MS));
        assert_eq!(opts.idle_threshold, Duration::from_secs(DEFAULT_IDLE_SECS));
        assert!(opts.exclude_patterns.iter().any(|p| p == "1password"));
    }

    #[test]
    fn matches_exclude_catches_password_manager() {
        let opts = UsageTrackerOpts::default();
        assert!(opts.matches_exclude("1Password.exe", Some("Vault")));
        assert!(opts.matches_exclude("chrome.exe", Some("Login — Password Manager")));
        assert!(!opts.matches_exclude("chrome.exe", Some("github.com")));
    }

    #[test]
    fn matches_exclude_picks_up_extra_patterns() {
        let opts = UsageTrackerOpts {
            poll_interval: Duration::from_millis(DEFAULT_POLL_MS),
            idle_threshold: Duration::from_secs(DEFAULT_IDLE_SECS),
            exclude_patterns: vec!["telegram".into(), "signal".into()],
        };
        assert!(opts.matches_exclude("telegram.exe", None));
        assert!(opts.matches_exclude("signal.exe", None));
        assert!(!opts.matches_exclude("chrome.exe", None));
    }
}
