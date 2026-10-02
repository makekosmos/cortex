// Usage tracker module — портированно из services/usage-tracker (Phase E1
// refactor). Запускается как tokio task внутри mundus-engine, пишет
// foreground-session-данные напрямую в ARK через ArkHost (in-process), без WS
// round-trip и без отдельного singleton-процесса.
//
// Что НЕ переехало (по сравнению со standalone usage-tracker):
//   * singleton lock — backend сам singleton (через SingletonGuard в main.rs),
//   * HKCU\Run installer — это shell installer ответственность,
//   * mundus_client.rs (WS) — мы внутри backend'а, прямой вызов ArkHost.
//   * spool — больше не нужен: нет WS round-trip и нет race с cold-start.

#[cfg(target_os = "windows")]
mod windows_capture;

#[cfg(windows)]
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::task::JoinHandle;

#[cfg(windows)]
use crate::app_index::{
    app::{App, AppKind},
    icons,
};
use crate::ark_host::ArkHost;

#[cfg(target_os = "windows")]
use windows_capture::{
    capture_foreground_window_with_diagnostics, process_window_state, ForegroundWindowSample,
    ProcessProbeCache, ProcessProbeDiagnostics, ProcessWindowState, WindowSnapshot, PLATFORM,
};

const TRACKER_DEVICE_ID_KEY: &str = "usage_tracker.device_id";
const DEFAULT_POLL_MS: u64 = 5_000;
const DEFAULT_IDLE_SECS: u64 = 180;
const SESSION_HEARTBEAT_FLUSH_MS: i64 = 60_000;
const USAGE_SPAN_HEARTBEAT_SECS: i64 = 60;
/// A persisted usage span may extend at most this far past its last flush:
/// span boundaries are wall-clock, so a stalled tick (sleep/hibernate) would
/// otherwise stretch the last foreground span across the whole sleep —
/// the same phantom-hours bug as session deltas (KOS-287).
const MAX_USAGE_SPAN_GAP_SECS: i64 = USAGE_SPAN_HEARTBEAT_SECS * 4;
const WINDOW_TITLE_STABILITY_MS: i64 = 10_000;
const HIDDEN_INACTIVE_SESSION_TTL_MS: i64 = 10 * 60_000;
const MAX_ACTIVE_SESSIONS: usize = 256;
const DIAGNOSTICS_LOG_INTERVAL: Duration = Duration::from_secs(60);

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
    pub icon_cache_dir: Option<PathBuf>,
    /// Lowercased substrings; sample матчится если ЛЮБОЙ из паттернов входит
    /// в process_name ИЛИ в window_title. Дефолт — DEFAULT_EXCLUDE_PATTERNS.
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Default)]
pub struct UsageTrackerDiagnosticsState {
    configured_enabled: AtomicBool,
    running: AtomicBool,
    tick_p95_ms: AtomicU64,
    active_sessions: AtomicUsize,
    enum_windows_calls_per_tick: AtomicU64,
    process_path_queries_per_tick: AtomicU64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageTrackerDiagnosticsSnapshot {
    pub configured_enabled: bool,
    pub running: bool,
    pub status: String,
    pub tick_p95_ms: u64,
    pub active_sessions: usize,
    pub enum_windows_calls_per_tick: u64,
    pub process_path_queries_per_tick: u64,
}

impl UsageTrackerDiagnosticsState {
    pub fn configure(&self, enabled: bool) {
        self.configured_enabled.store(enabled, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn mark_running(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    pub fn mark_stopped(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn observe(
        &self,
        tick_p95_ms: u64,
        active_sessions: usize,
        enum_windows_calls_per_tick: u64,
        process_path_queries_per_tick: u64,
    ) {
        self.tick_p95_ms.store(tick_p95_ms, Ordering::SeqCst);
        self.active_sessions
            .store(active_sessions, Ordering::SeqCst);
        self.enum_windows_calls_per_tick
            .store(enum_windows_calls_per_tick, Ordering::SeqCst);
        self.process_path_queries_per_tick
            .store(process_path_queries_per_tick, Ordering::SeqCst);
    }

    pub fn snapshot(&self) -> UsageTrackerDiagnosticsSnapshot {
        let configured_enabled = self.configured_enabled.load(Ordering::SeqCst);
        let running = self.running.load(Ordering::SeqCst);
        UsageTrackerDiagnosticsSnapshot {
            configured_enabled,
            running,
            status: if !configured_enabled {
                "disabled".to_string()
            } else if running {
                "running".to_string()
            } else {
                "starting".to_string()
            },
            tick_p95_ms: self.tick_p95_ms.load(Ordering::SeqCst),
            active_sessions: self.active_sessions.load(Ordering::SeqCst),
            enum_windows_calls_per_tick: self.enum_windows_calls_per_tick.load(Ordering::SeqCst),
            process_path_queries_per_tick: self
                .process_path_queries_per_tick
                .load(Ordering::SeqCst),
        }
    }
}

impl Default for UsageTrackerOpts {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(DEFAULT_POLL_MS),
            idle_threshold: Duration::from_secs(DEFAULT_IDLE_SECS),
            icon_cache_dir: None,
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
        // MUNDUS_USAGE_TRACKER_EXCLUDE_EXTRA — comma-separated user добавки
        // поверх дефолтного списка. Пустая строка / отсутствие = только дефолт.
        let mut excludes: Vec<String> = DEFAULT_EXCLUDE_PATTERNS
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        if let Ok(extra) = std::env::var("MUNDUS_USAGE_TRACKER_EXCLUDE_EXTRA") {
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
            icon_cache_dir: None,
            exclude_patterns: excludes,
        }
    }

    pub fn with_icon_cache_dir(mut self, icon_cache_dir: PathBuf) -> Self {
        self.icon_cache_dir = Some(icon_cache_dir);
        self
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
pub fn spawn(
    ark: Arc<ArkHost>,
    opts: UsageTrackerOpts,
    diagnostics: Arc<UsageTrackerDiagnosticsState>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(e) = run(Arc::clone(&ark), opts.clone(), diagnostics.clone()).await {
                eprintln!("[usage-tracker] loop crashed, restarting in 5s: {e}");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    })
}

#[cfg(not(target_os = "windows"))]
pub fn spawn(
    _ark: Arc<ArkHost>,
    _opts: UsageTrackerOpts,
    _diagnostics: Arc<UsageTrackerDiagnosticsState>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        eprintln!("[usage-tracker] disabled on non-Windows platforms");
    })
}

#[cfg(target_os = "windows")]
async fn run(
    ark: Arc<ArkHost>,
    opts: UsageTrackerOpts,
    diagnostics_state: Arc<UsageTrackerDiagnosticsState>,
) -> Result<(), String> {
    let identity = TrackerIdentity {
        device_id: load_or_create_tracker_device_id(&ark).await?,
        device_name: resolve_device_name(),
    };
    eprintln!(
        "[usage-tracker] started: device_id={} device_name={:?}",
        identity.device_id, identity.device_name
    );

    let mut active_sessions: HashMap<SessionKey, ActiveSession> = HashMap::new();
    let mut process_probe_cache = ProcessProbeCache::default();
    let mut diagnostics_reporter = UsageTrackerDiagnostics::new(diagnostics_state);
    let mut previous_tick = std::time::Instant::now();
    // first_seen_at cache: stable между запусками backend'а сохраняется через
    // upsert (DB) — но в рамках одного процесса нам достаточно in-memory HashMap,
    // потому что мы первый раз увидим приложение в этой сессии и зафиксируем
    // captured_at; последующие session_started для того же приложения возьмут
    // ту же дату. Между перезапусками backend'а first_seen_at будет «сегодня» —
    // приемлемо, regression от прежнего поведения минимальная (см. proposal 8.4).
    let mut first_seen_cache: HashMap<String, String> = HashMap::new();
    let mut active_usage_span: Option<ActiveUsageSpan> = None;

    loop {
        let tick_started = std::time::Instant::now();
        let idle = opts.idle_threshold;
        let tick_probe = match tokio::task::spawn_blocking(move || {
            let mut diagnostics = ProcessProbeDiagnostics::default();
            let raw_sample =
                match capture_foreground_window_with_diagnostics(idle, &mut diagnostics) {
                    Ok(sample) => sample,
                    Err(error) => {
                        eprintln!("[usage-tracker] capture failed: {error}");
                        None
                    }
                };
            let window_snapshot = WindowSnapshot::capture(&mut diagnostics);
            TickProbe {
                raw_sample,
                window_snapshot,
                diagnostics,
            }
        })
        .await
        {
            Ok(probe) => probe,
            Err(error) => {
                eprintln!("[usage-tracker] capture join failed: {error}");
                TickProbe {
                    raw_sample: None,
                    window_snapshot: WindowSnapshot::default(),
                    diagnostics: ProcessProbeDiagnostics::default(),
                }
            }
        };
        let mut window_snapshot = tick_probe.window_snapshot;
        let mut probe_diagnostics = tick_probe.diagnostics;
        // Privacy filter — password manager'ы и подобные не пишем в БД.
        // Treat'им как «нет foreground окна»: excluded process не стартует,
        // уже известные non-excluded процессы продолжают runtime tracking.
        let sample = tick_probe
            .raw_sample
            .filter(|s| !opts.matches_exclude(&s.process_name, s.window_title.as_deref()));
        let captured_now = chrono::Utc::now();
        let captured_at = captured_now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let captured_at_unix = captured_now.timestamp();
        let delta_ms = capped_tick_delta_ms(previous_tick.elapsed(), opts.poll_interval);
        let foreground_key = sample.as_ref().map(SessionKey::from_sample);

        let mut ended_keys = Vec::new();
        for (key, session) in active_sessions.iter_mut() {
            let window_state = process_window_state_or_dead(
                key.pid,
                &session.tracked_app.normalized_exe_path,
                &mut window_snapshot,
                &mut process_probe_cache,
                &mut probe_diagnostics,
            );
            if window_state == ProcessWindowState::Dead {
                ended_keys.push((key.clone(), "session_ended"));
                continue;
            }
            session.accumulate(
                delta_ms,
                foreground_key.as_ref() == Some(key),
                window_state == ProcessWindowState::AliveVisible,
            );
            if session.should_finalize_hidden_inactive(HIDDEN_INACTIVE_SESSION_TTL_MS) {
                ended_keys.push((key.clone(), "session_hidden_timeout"));
                continue;
            }
            if session.should_flush_heartbeat(SESSION_HEARTBEAT_FLUSH_MS) {
                if let Err(error) =
                    flush_session_heartbeat(&ark, &identity, session, &identity.device_id).await
                {
                    eprintln!("[usage-tracker] persist session heartbeat failed: {error}");
                }
            }
        }

        if let Some(sample) = sample {
            let key = SessionKey::from_sample(&sample);
            if let Some(active) = active_sessions.get_mut(&key) {
                update_active_session(active, sample, captured_at.clone(), delta_ms);
            } else {
                match start_session(
                    &ark,
                    &identity,
                    &mut first_seen_cache,
                    sample,
                    captured_at.clone(),
                    opts.icon_cache_dir.clone(),
                )
                .await
                {
                    Ok(session) => {
                        active_sessions.insert(key, session);
                        for overflow_key in overflow_session_keys(&active_sessions) {
                            ended_keys.push((overflow_key, "session_pruned"));
                        }
                    }
                    Err(error) => eprintln!("[usage-tracker] start session failed: {error}"),
                }
            }
        }

        for (key, kind) in ended_keys {
            if let Some(active) = active_sessions.remove(&key) {
                if let Err(error) =
                    finalize_session(&ark, &identity, &active, captured_at.clone(), kind).await
                {
                    eprintln!("[usage-tracker] finalize session failed: {error}");
                }
            }
        }
        let timeline_key = foreground_key
            .as_ref()
            .and_then(|key| active_sessions.get(key))
            .map(UsageTimelineKey::from_session);
        if let Err(error) = update_usage_timeline(
            &ark,
            &identity,
            &mut active_usage_span,
            timeline_key,
            captured_at_unix,
            &captured_at,
        )
        .await
        {
            eprintln!("[usage-tracker] persist compact usage span failed: {error}");
        }
        let live_pids = active_sessions
            .keys()
            .map(|key| key.pid)
            .collect::<HashSet<_>>();
        process_probe_cache.retain_pids(&live_pids);

        previous_tick = std::time::Instant::now();
        diagnostics_reporter.observe(
            tick_started.elapsed().as_millis().min(i64::MAX as u128) as i64,
            active_sessions.len(),
            &probe_diagnostics,
        );
        tokio::time::sleep(opts.poll_interval).await;
    }
}

/// Per-tick attribution cap. `delta_ms` is the real elapsed time since the
/// previous tick; on a stall (sleep, hibernate, debugger pause) the full gap
/// would be credited to every tracked session — and to whatever window
/// happened to stay focused. Ticks land at ~poll_interval, so 4× poll (with a
/// 60s floor) covers scheduling jitter without letting a stall mint phantom
/// hours (KOS-287).
fn capped_tick_delta_ms(elapsed: Duration, poll_interval: Duration) -> i64 {
    let cap_ms = (poll_interval.as_millis() * 4)
        .max(60_000)
        .min(i64::MAX as u128) as i64;
    (elapsed.as_millis().min(i64::MAX as u128) as i64).min(cap_ms)
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct TickProbe {
    raw_sample: Option<ForegroundWindowSample>,
    window_snapshot: WindowSnapshot,
    diagnostics: ProcessProbeDiagnostics,
}

#[derive(Debug, Clone)]
struct TrackerIdentity {
    device_id: String,
    device_name: String,
}

#[cfg(target_os = "windows")]
fn process_window_state_or_dead(
    pid: u32,
    expected_normalized_exe_path: &str,
    snapshot: &mut WindowSnapshot,
    cache: &mut ProcessProbeCache,
    diagnostics: &mut ProcessProbeDiagnostics,
) -> ProcessWindowState {
    process_window_state_from_probe_result(
        pid,
        process_window_state(
            pid,
            expected_normalized_exe_path,
            snapshot,
            cache,
            diagnostics,
        ),
    )
}

#[cfg(target_os = "windows")]
fn process_window_state_from_probe_result(
    pid: u32,
    result: Result<ProcessWindowState, String>,
) -> ProcessWindowState {
    match result {
        Ok(state) => state,
        Err(error) => {
            eprintln!("[usage-tracker] process window check failed for pid {pid}: {error}");
            // См. postmortems.md § 2026-06-01 — Usage tracker спамит process window check failed.
            ProcessWindowState::Dead
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SessionKey {
    tracked_app_id: String,
    pid: u32,
}

#[cfg(target_os = "windows")]
impl SessionKey {
    fn from_sample(sample: &ForegroundWindowSample) -> Self {
        Self {
            tracked_app_id: sample.tracked_app_id.clone(),
            pid: sample.pid,
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct ActiveSession {
    tracked_app: TrackedAppFields,
    session_id: String,
    started_at: String,
    runtime_ms: i64,
    foreground_ms: i64,
    idle_ms: i64,
    window_title: Option<String>,
    persisted_window_title: Option<String>,
    pending_window_title: Option<PendingWindowTitle>,
    process_name: String,
    exe_path: String,
    pid_start: i64,
    pid_end: i64,
    current_is_foreground: bool,
    current_is_visible: bool,
    current_is_idle: bool,
    current_is_private: bool,
    sample_count: u64,
    heartbeat_elapsed_ms: i64,
    hidden_inactive_elapsed_ms: i64,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct PendingWindowTitle {
    title: Option<String>,
    first_seen_at: String,
    stable_ms: i64,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct UsageTimelineKey {
    tracked_app_id: String,
    window_title: Option<String>,
    is_idle: bool,
    is_private: bool,
}

#[cfg(target_os = "windows")]
impl UsageTimelineKey {
    fn from_session(session: &ActiveSession) -> Self {
        Self {
            tracked_app_id: session.tracked_app.id.clone(),
            window_title: session.persisted_window_title.clone(),
            is_idle: session.current_is_idle,
            is_private: session.current_is_private,
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct ActiveUsageSpan {
    key: UsageTimelineKey,
    started_at_unix: i64,
    last_flushed_at_unix: i64,
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
    icon_ref: Option<String>,
    first_seen_at: String,
    last_seen_at: String,
}

#[cfg(target_os = "windows")]
impl ActiveSession {
    fn matches(&self, sample: &ForegroundWindowSample) -> bool {
        self.tracked_app.id == sample.tracked_app_id && self.pid_end == i64::from(sample.pid)
    }

    fn accumulate(&mut self, delta_ms: i64, is_foreground: bool, is_visible: bool) {
        self.current_is_foreground = is_foreground;
        self.current_is_visible = is_visible;
        if is_visible || is_foreground {
            self.hidden_inactive_elapsed_ms = 0;
        } else {
            self.hidden_inactive_elapsed_ms = self
                .hidden_inactive_elapsed_ms
                .saturating_add(delta_ms.max(0));
        }
        if is_visible {
            self.runtime_ms += delta_ms;
        }
        if is_visible && is_foreground && self.current_is_idle {
            self.idle_ms += delta_ms;
        } else if is_visible && is_foreground {
            self.foreground_ms += delta_ms;
        }
        self.sample_count += 1;
        self.heartbeat_elapsed_ms = self.heartbeat_elapsed_ms.saturating_add(delta_ms);
    }

    fn should_flush_heartbeat(&self, flush_interval_ms: i64) -> bool {
        self.heartbeat_elapsed_ms >= flush_interval_ms
    }

    fn mark_heartbeat_flushed(&mut self) {
        self.heartbeat_elapsed_ms = 0;
    }

    fn should_finalize_hidden_inactive(&self, ttl_ms: i64) -> bool {
        !self.current_is_visible
            && !self.current_is_foreground
            && self.hidden_inactive_elapsed_ms >= ttl_ms
    }

    fn observe_window_title(
        &mut self,
        title: Option<String>,
        captured_at: String,
        elapsed_ms: i64,
        is_idle: bool,
        is_private: bool,
    ) -> Option<String> {
        self.window_title = title.clone();
        if is_private {
            if title == self.persisted_window_title {
                self.pending_window_title = None;
                return None;
            }
            self.persisted_window_title = title;
            self.pending_window_title = None;
            return Some(captured_at);
        }
        if is_idle || title == self.persisted_window_title {
            self.pending_window_title = None;
            return None;
        }

        match self.pending_window_title.as_mut() {
            Some(pending) if pending.title == title => {
                pending.stable_ms = pending.stable_ms.saturating_add(elapsed_ms.max(0));
                if pending.stable_ms < WINDOW_TITLE_STABILITY_MS {
                    return None;
                }
                Some(pending.first_seen_at.clone())
            }
            _ => {
                self.pending_window_title = Some(PendingWindowTitle {
                    title,
                    first_seen_at: captured_at,
                    stable_ms: 0,
                });
                None
            }
        }
    }

    fn mark_window_title_persisted(&mut self) {
        self.persisted_window_title = self.window_title.clone();
        self.pending_window_title = None;
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
            "runtimeMs": self.runtime_ms,
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
                "runtimeMs": self.runtime_ms,
                "visible": self.current_is_visible,
            },
        })
    }
}

#[cfg(target_os = "windows")]
fn update_active_session(
    active: &mut ActiveSession,
    sample: ForegroundWindowSample,
    captured_at: String,
    elapsed_ms: i64,
) {
    debug_assert!(active.matches(&sample));
    active.tracked_app.last_seen_at = captured_at.clone();
    active.current_is_foreground = true;
    active.current_is_idle = sample.is_idle;
    active.current_is_private = sample.is_private;
    active.pid_end = i64::from(sample.pid);

    if active
        .observe_window_title(
            sample.window_title.clone(),
            captured_at,
            elapsed_ms,
            sample.is_idle,
            sample.is_private,
        )
        .is_some()
    {
        active.mark_window_title_persisted();
    }
}

#[cfg(target_os = "windows")]
async fn start_session(
    ark: &Arc<ArkHost>,
    identity: &TrackerIdentity,
    first_seen_cache: &mut HashMap<String, String>,
    sample: ForegroundWindowSample,
    captured_at: String,
    icon_cache_dir: Option<PathBuf>,
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
        display_name: resolve_display_name(&sample.exe_path, &sample.process_name).await,
        icon_ref: resolve_icon_ref(&sample, icon_cache_dir).await,
        first_seen_at,
        last_seen_at: captured_at.clone(),
    };

    persist_tracked_app(ark, &tracked_app, &identity.device_id).await?;

    let session = ActiveSession {
        tracked_app,
        session_id: new_uuid(),
        started_at: captured_at.clone(),
        runtime_ms: 0,
        foreground_ms: 0,
        idle_ms: 0,
        window_title: sample.window_title.clone(),
        persisted_window_title: sample.window_title.clone(),
        pending_window_title: None,
        process_name: sample.process_name.clone(),
        exe_path: sample.exe_path.clone(),
        pid_start: i64::from(sample.pid),
        pid_end: i64::from(sample.pid),
        current_is_foreground: true,
        current_is_visible: true,
        current_is_idle: sample.is_idle,
        current_is_private: sample.is_private,
        sample_count: 1,
        heartbeat_elapsed_ms: 0,
        hidden_inactive_elapsed_ms: 0,
    };

    persist_usage_session(
        ark,
        &session.to_usage_session(identity, None),
        &identity.device_id,
    )
    .await?;
    Ok(session)
}

#[cfg(target_os = "windows")]
fn overflow_session_keys(active_sessions: &HashMap<SessionKey, ActiveSession>) -> Vec<SessionKey> {
    if active_sessions.len() <= MAX_ACTIVE_SESSIONS {
        return Vec::new();
    }

    let mut candidates = active_sessions
        .iter()
        .filter(|(_, session)| !session.current_is_foreground)
        .map(|(key, session)| {
            (
                key.clone(),
                session.current_is_visible,
                session.hidden_inactive_elapsed_ms,
                session.started_at.clone(),
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.3.cmp(&b.3))
    });

    candidates
        .into_iter()
        .take(active_sessions.len().saturating_sub(MAX_ACTIVE_SESSIONS))
        .map(|(key, _, _, _)| key)
        .collect()
}

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
struct UsageTrackerDiagnostics {
    state: Arc<UsageTrackerDiagnosticsState>,
    last_log_at: Option<std::time::Instant>,
    tick_ms_samples: Vec<i64>,
}

#[cfg(target_os = "windows")]
impl UsageTrackerDiagnostics {
    fn new(state: Arc<UsageTrackerDiagnosticsState>) -> Self {
        Self {
            state,
            last_log_at: None,
            tick_ms_samples: Vec::new(),
        }
    }

    fn observe(
        &mut self,
        tick_ms: i64,
        active_sessions: usize,
        probe_diagnostics: &ProcessProbeDiagnostics,
    ) {
        let now = std::time::Instant::now();
        self.tick_ms_samples.push(tick_ms.max(0));
        let should_log = self
            .last_log_at
            .map(|last| last.elapsed() >= DIAGNOSTICS_LOG_INTERVAL)
            .unwrap_or(true);
        if !should_log {
            return;
        }

        let p95 = percentile(&mut self.tick_ms_samples, 95);
        self.state.observe(
            p95.max(0) as u64,
            active_sessions,
            u64::from(probe_diagnostics.enum_windows_calls),
            u64::from(probe_diagnostics.process_path_queries),
        );
        eprintln!(
            "[usage-tracker] diagnostics usage_tracker.tick_ms={} usage_tracker.tick_ms.p95={} \
                 usage_tracker.active_sessions={} usage_tracker.enum_windows_calls_per_tick={} \
                 usage_tracker.process_path_queries_per_tick={} \
                 usage_tracker.visible_window_count={}",
            tick_ms,
            p95,
            active_sessions,
            probe_diagnostics.enum_windows_calls,
            probe_diagnostics.process_path_queries,
            probe_diagnostics.visible_window_count,
        );
        self.last_log_at = Some(now);
        self.tick_ms_samples.clear();
    }
}

#[cfg(target_os = "windows")]
fn percentile(samples: &mut [i64], percentile: usize) -> i64 {
    if samples.is_empty() {
        return 0;
    }
    samples.sort_unstable();
    let rank = ((samples.len() - 1) * percentile) / 100;
    samples[rank]
}

#[cfg(target_os = "windows")]
async fn persist_compact_usage_span(
    ark: &Arc<ArkHost>,
    identity: &TrackerIdentity,
    span: &ActiveUsageSpan,
    ended_at_unix: i64,
    updated_at: &str,
) -> Result<(), String> {
    if ended_at_unix <= span.started_at_unix {
        return Ok(());
    }
    let flags = i64::from(span.key.is_idle) | (i64::from(span.key.is_private) << 1);
    ark_request(
        ark,
        "upsert_usage_span",
        json!({
            "usage_span": {
                "deviceId": identity.device_id,
                "startedAtUnix": span.started_at_unix,
                "endedAtUnix": ended_at_unix,
                "trackedAppId": span.key.tracked_app_id,
                "windowTitle": span.key.window_title,
                "flags": flags,
                "updatedAt": updated_at,
            }
        }),
    )
    .await
    .map(|_| ())
}

#[cfg(target_os = "windows")]
async fn update_usage_timeline(
    ark: &Arc<ArkHost>,
    identity: &TrackerIdentity,
    active: &mut Option<ActiveUsageSpan>,
    current: Option<UsageTimelineKey>,
    now_unix: i64,
    updated_at: &str,
) -> Result<(), String> {
    let state_changed = active.as_ref().map(|span| &span.key) != current.as_ref();
    if state_changed {
        if let Some(previous) = active.as_ref() {
            let ended_at = clamp_span_end(previous, now_unix);
            persist_compact_usage_span(ark, identity, previous, ended_at, updated_at).await?;
        }
        *active = current.map(|key| ActiveUsageSpan {
            key,
            started_at_unix: now_unix,
            last_flushed_at_unix: now_unix,
        });
        return Ok(());
    }

    if let Some(span) = active.as_mut() {
        if now_unix.saturating_sub(span.last_flushed_at_unix) >= USAGE_SPAN_HEARTBEAT_SECS {
            let ended_at = clamp_span_end(span, now_unix);
            persist_compact_usage_span(ark, identity, span, ended_at, updated_at).await?;
            span.last_flushed_at_unix = now_unix;
        }
    }
    Ok(())
}

/// Wall-clock span end, clamped to at most MAX_USAGE_SPAN_GAP_SECS past the
/// last flush — a stalled/suspended tick must not stretch the span (KOS-287).
#[cfg(target_os = "windows")]
fn clamp_span_end(span: &ActiveUsageSpan, now_unix: i64) -> i64 {
    now_unix
        .min(
            span.last_flushed_at_unix
                .saturating_add(MAX_USAGE_SPAN_GAP_SECS),
        )
        .max(span.started_at_unix)
}

#[cfg(target_os = "windows")]
async fn flush_session_heartbeat(
    ark: &Arc<ArkHost>,
    identity: &TrackerIdentity,
    active: &mut ActiveSession,
    device_id: &str,
) -> Result<(), String> {
    // См. postmortems.md § 2026-06-06. Per-poll writes bump ARK sync state and
    // keep the ARK service hot; heartbeat is a bounded durability checkpoint. Do
    // not persist tracked_app here: last_seen_at can wait until session end, and
    // each extra upsert rewrites the sync version vector on large dev DBs.
    persist_usage_session(ark, &active.to_usage_session(identity, None), device_id).await?;
    active.mark_heartbeat_flushed();
    Ok(())
}

#[cfg(target_os = "windows")]
async fn finalize_session(
    ark: &Arc<ArkHost>,
    identity: &TrackerIdentity,
    active: &ActiveSession,
    ended_at: String,
    _kind: &str,
) -> Result<(), String> {
    persist_tracked_app(ark, &active.tracked_app, &identity.device_id).await?;
    persist_usage_session(
        ark,
        &active.to_usage_session(identity, Some(ended_at.clone())),
        &identity.device_id,
    )
    .await?;
    Ok(())
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
    let existing = ark_request(ark, "get_sync_kv", json!({ "key": TRACKER_DEVICE_ID_KEY })).await?;
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
        "iconRef": tracked_app.icon_ref,
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

// ----- utils -----

#[cfg(target_os = "windows")]
async fn resolve_icon_ref(
    sample: &ForegroundWindowSample,
    icon_cache_dir: Option<PathBuf>,
) -> Option<String> {
    let icon_cache_dir = icon_cache_dir?;
    let app = App {
        id: sample.tracked_app_id.clone(),
        name: sample
            .window_title
            .clone()
            .unwrap_or_else(|| sample.process_name.clone()),
        exec_path: sample.exe_path.clone(),
        icon_path: None,
        icon_source: None,
        kind: AppKind::Win32,
        source: "usage_tracker".to_string(),
        mtime: 0,
    };

    tokio::task::spawn_blocking(move || icons::ensure_icon(&icon_cache_dir, &app).ok())
        .await
        .ok()
        .flatten()
}

fn resolve_device_name() -> String {
    std::env::var("MUNDUS_DEVICE_NAME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "Windows Device".to_string())
}

/// Display name comes from the exe version resource (FileDescription →
/// ProductName → file stem) — never the window title, or a browser would be
/// labelled with whatever tab happened to be open last (KOS-287).
#[cfg(target_os = "windows")]
async fn resolve_display_name(exe_path: &str, process_name: &str) -> Option<String> {
    let path = exe_path.to_string();
    let name =
        tokio::task::spawn_blocking(move || crate::app_index::exe_info::exe_display_name(&path))
            .await
            .ok()
            .flatten();
    name.or_else(|| {
        std::path::Path::new(process_name)
            .file_stem()
            .and_then(|value| value.to_str())
            .map(|value| value.to_string())
    })
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
    fn diagnostics_distinguish_disabled_from_running() {
        let state = UsageTrackerDiagnosticsState::default();
        assert_eq!(state.snapshot().status, "disabled");
        state.configure(true);
        assert_eq!(state.snapshot().status, "starting");
        state.mark_running();
        assert_eq!(state.snapshot().status, "running");
        state.configure(false);
        assert_eq!(state.snapshot().status, "disabled");
    }

    #[test]
    fn opts_from_env_uses_defaults_when_unset() {
        std::env::remove_var("USAGE_TRACKER_POLL_MS");
        std::env::remove_var("USAGE_TRACKER_IDLE_SECS");
        std::env::remove_var("MUNDUS_USAGE_TRACKER_EXCLUDE_EXTRA");
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
            icon_cache_dir: None,
            exclude_patterns: vec!["telegram".into(), "signal".into()],
        };
        assert!(opts.matches_exclude("telegram.exe", None));
        assert!(opts.matches_exclude("signal.exe", None));
        assert!(!opts.matches_exclude("chrome.exe", None));
    }

    #[cfg(target_os = "windows")]
    fn test_session() -> ActiveSession {
        ActiveSession {
            tracked_app: TrackedAppFields {
                id: "app-1".to_string(),
                platform: PLATFORM.to_string(),
                exe_path: r"C:\Games\Demo\demo.exe".to_string(),
                normalized_exe_path: r"c:\games\demo\demo.exe".to_string(),
                process_name: "demo.exe".to_string(),
                display_name: Some("Demo".to_string()),
                icon_ref: None,
                first_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
                last_seen_at: "2026-01-01T00:00:00.000Z".to_string(),
            },
            session_id: "session-1".to_string(),
            started_at: "2026-01-01T00:00:00.000Z".to_string(),
            runtime_ms: 0,
            foreground_ms: 0,
            idle_ms: 0,
            window_title: Some("Demo".to_string()),
            persisted_window_title: Some("Demo".to_string()),
            pending_window_title: None,
            process_name: "demo.exe".to_string(),
            exe_path: r"C:\Games\Demo\demo.exe".to_string(),
            pid_start: 100,
            pid_end: 100,
            current_is_foreground: true,
            current_is_visible: true,
            current_is_idle: false,
            current_is_private: false,
            sample_count: 0,
            heartbeat_elapsed_ms: 0,
            hidden_inactive_elapsed_ms: 0,
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn active_session_counts_background_runtime_without_foreground() {
        let mut session = test_session();
        session.accumulate(1_000, true, true);
        session.accumulate(2_000, false, true);

        assert_eq!(session.runtime_ms, 3_000);
        assert_eq!(session.foreground_ms, 1_000);
        assert_eq!(session.idle_ms, 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn active_session_counts_idle_foreground_separately_from_runtime() {
        let mut session = test_session();
        session.current_is_idle = true;
        session.accumulate(1_000, true, true);
        session.accumulate(2_000, false, true);

        assert_eq!(session.runtime_ms, 3_000);
        assert_eq!(session.foreground_ms, 0);
        assert_eq!(session.idle_ms, 1_000);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn active_session_does_not_count_hidden_runtime() {
        let mut session = test_session();
        session.accumulate(1_000, true, true);
        session.accumulate(5_000, false, false);

        assert_eq!(session.runtime_ms, 1_000);
        assert_eq!(session.foreground_ms, 1_000);
        assert_eq!(session.idle_ms, 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn hidden_inactive_session_expires_after_ttl() {
        // Regression: 2026-06-08. Hidden sessions must not accumulate forever
        // and force per-tick Win32 probes for the rest of the day.
        let mut session = test_session();
        session.accumulate(HIDDEN_INACTIVE_SESSION_TTL_MS - 1, false, false);
        assert!(!session.should_finalize_hidden_inactive(HIDDEN_INACTIVE_SESSION_TTL_MS));

        session.accumulate(1, false, false);
        assert!(session.should_finalize_hidden_inactive(HIDDEN_INACTIVE_SESSION_TTL_MS));

        session.accumulate(1_000, false, true);
        assert!(!session.should_finalize_hidden_inactive(HIDDEN_INACTIVE_SESSION_TTL_MS));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn overflow_session_keys_prunes_hidden_background_before_visible() {
        // Regression: 2026-06-08. A hard cap keeps the active session map
        // bounded even if many processes stay alive after losing all windows.
        let mut sessions = HashMap::new();
        for index in 0..=MAX_ACTIVE_SESSIONS {
            let mut session = test_session();
            session.session_id = format!("session-{index}");
            session.started_at = format!("2026-01-01T00:00:{index:02}.000Z");
            session.current_is_foreground = false;
            session.current_is_visible = index % 2 == 0;
            session.hidden_inactive_elapsed_ms = index as i64;
            sessions.insert(
                SessionKey {
                    tracked_app_id: format!("app-{index}"),
                    pid: index as u32 + 100,
                },
                session,
            );
        }

        let pruned = overflow_session_keys(&sessions);
        assert_eq!(pruned.len(), 1);
        let pruned_session = sessions.get(&pruned[0]).expect("pruned session exists");
        assert!(!pruned_session.current_is_visible);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn active_session_heartbeat_flush_is_rate_limited() {
        // Regression: 2026-06-06. Persisting the session every 1s poll kept
        // ARK sync/version-vector hot while the foreground window was stable.
        let mut session = test_session();
        for _ in 0..59 {
            session.accumulate(1_000, true, true);
            assert!(!session.should_flush_heartbeat(SESSION_HEARTBEAT_FLUSH_MS));
        }

        session.accumulate(1_000, true, true);
        assert!(session.should_flush_heartbeat(SESSION_HEARTBEAT_FLUSH_MS));

        session.mark_heartbeat_flushed();
        assert!(!session.should_flush_heartbeat(SESSION_HEARTBEAT_FLUSH_MS));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_title_is_persisted_only_after_ten_stable_seconds() {
        fn observe(
            session: &mut ActiveSession,
            title: &str,
            captured_at: &str,
            elapsed_ms: i64,
        ) -> Option<String> {
            session.observe_window_title(
                Some(title.to_string()),
                captured_at.to_string(),
                elapsed_ms,
                false,
                false,
            )
        }

        let mut session = test_session();
        assert_eq!(
            observe(&mut session, "Flash", "2026-01-01T00:00:01Z", 1_000),
            None
        );
        assert_eq!(
            observe(&mut session, "Demo", "2026-01-01T00:00:02Z", 1_000),
            None
        );
        let first_seen = "2026-01-01T00:00:03.000Z";
        assert_eq!(observe(&mut session, "GitHub", first_seen, 1_000), None);
        assert_eq!(
            observe(&mut session, "GitHub", "2026-01-01T00:00:12Z", 9_000),
            None
        );
        assert_eq!(
            observe(&mut session, "GitHub", "2026-01-01T00:00:13Z", 1_000),
            Some(first_seen.to_string())
        );
        session.mark_window_title_persisted();
        assert_eq!(
            observe(&mut session, "GitHub", "2026-01-01T00:00:14Z", 1_000),
            None
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn active_time_is_focus_minus_idle_not_visible_runtime() {
        // KOS-287 regression, the Raycast case: a launcher sits visible in the
        // background for hours but is focused for seconds. Active time is
        // foreground&&!idle only — runtime (visible) stays a separate counter.
        let mut session = test_session();
        for _ in 0..100 {
            session.accumulate(60_000, false, true); // visible, not focused
        }
        session.current_is_idle = false;
        session.accumulate(5_000, true, true); // briefly focused
        session.current_is_idle = true;
        session.accumulate(300_000, true, true); // focused but idle

        assert_eq!(session.foreground_ms, 5_000);
        assert_eq!(session.idle_ms, 300_000);
        assert_eq!(session.runtime_ms, 6_305_000);
    }

    #[test]
    fn tick_delta_is_capped_so_stalls_do_not_mint_phantom_time() {
        // KOS-287 regression: a suspended tracker must not credit the whole
        // gap. Normal ticks stay untouched; an 8h sleep collapses to the cap.
        let poll = Duration::from_millis(DEFAULT_POLL_MS);
        assert_eq!(
            capped_tick_delta_ms(Duration::from_millis(5_000), poll),
            5_000
        );
        assert_eq!(
            capped_tick_delta_ms(Duration::from_millis(45_000), poll),
            45_000
        );
        assert_eq!(
            capped_tick_delta_ms(Duration::from_secs(8 * 3600), poll),
            60_000
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn usage_span_end_is_capped_past_last_flush() {
        let span = ActiveUsageSpan {
            key: UsageTimelineKey {
                tracked_app_id: "app".to_string(),
                window_title: None,
                is_idle: false,
                is_private: false,
            },
            started_at_unix: 1_000,
            last_flushed_at_unix: 1_060,
        };
        // Normal heartbeat pace — end follows now.
        assert_eq!(clamp_span_end(&span, 1_120), 1_120);
        // Stall: end is clamped to last_flush + MAX_USAGE_SPAN_GAP_SECS.
        assert_eq!(clamp_span_end(&span, 1_060 + 8 * 3_600), 1_300);
        // Clock skew backwards never produces an end before the start.
        assert_eq!(clamp_span_end(&span, 100), 1_000);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn process_window_probe_error_ends_session() {
        // Regression: 2026-06-01. Win32 probe errors for stale PIDs must not keep
        // sessions alive forever while logging every poll tick.
        let state = process_window_state_from_probe_result(
            1364,
            Err("Присоединенное к системе устройство не работает. (0x8007001F)".to_string()),
        );

        assert_eq!(state, ProcessWindowState::Dead);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn percentile_uses_sorted_rank() {
        let mut samples = vec![10, 1, 7, 3, 5];
        assert_eq!(percentile(&mut samples, 95), 7);
    }
}
