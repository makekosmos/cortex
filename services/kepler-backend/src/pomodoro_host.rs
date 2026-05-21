// Pomodoro host: singleton Session живёт в backend'е (Arc<Mutex<...>>) +
// tokio ticker который раз в секунду дёргает Session::tick(). События
// сериализуются и форвардятся всем connected WS-клиентам через broadcast.
//
// Wire format flat (см. запреты forbidden.md: никаких nested {kind,type,payload}):
//   {"event":"pomodoro_tick", "phase":"work", "remainingMs":..., "phaseEndsAtMs":<unix ms|null>, ...state}
//   {"event":"pomodoro_phase_changed", "from":"work", "to":"shortBreak", ...state}
//   {"event":"pomodoro_finished", "finished":"work", ...state}
//
// `phaseEndsAtMs` — wallclock end of current phase (`Some` только когда
// running && !paused). Renderer интерполирует remainingMs локально по
// этому полю — backend тикает 1 Hz и UI smoothness не зависит от tick rate.
//
// ARK ops dispatched в ws_server (см. handle_pomodoro_op):
//   pomodoro.start { config }
//   pomodoro.pause
//   pomodoro.resume
//   pomodoro.skip
//   pomodoro.stop
//   pomodoro.get_state

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ark_core::pomodoro::{
    Clock, PersistedSession, Phase, Session, SessionConfig, SessionEvent, SystemClock,
};
use serde_json::{json, Value};
use tokio::sync::{broadcast, Mutex};

/// Basename файла с persisted pomodoro state. Полный путь — `<data_dir>/<this>`.
const STATE_FILE_BASENAME: &str = "pomodoro-state.json";

/// Snake-case string repr для wire-формата. ark-core's `Phase` сериализуется
/// в camelCase (idle/work/shortBreak/longBreak) — это уже совпадает с TS.
fn phase_to_str(p: Phase) -> &'static str {
    match p {
        Phase::Idle => "idle",
        Phase::Work => "work",
        Phase::ShortBreak => "shortBreak",
        Phase::LongBreak => "longBreak",
    }
}

/// PomodoroHost — singleton в kepler-backend. Внутри Arc<Mutex<Session>> +
/// broadcast channel с уже-форматированными WS-events (JSON Value).
pub struct PomodoroHost {
    session: Arc<Mutex<Session>>,
    /// Готовые-к-отправке JSON events. WS-клиенты subscribe в этот канал
    /// и форвардят строки в свои socket'ы.
    events_tx: broadcast::Sender<Value>,
    /// Базовая директория для persistence. `None` отключает file I/O — для
    /// in-memory unit-тестов.
    data_dir: Option<PathBuf>,
}

impl PomodoroHost {
    /// Создаёт хост с persistence в `<data_dir>/pomodoro-state.json`. На
    /// startup пробует загрузить state и реставрировать Session (с advance
    /// истёкших фаз). Если persistence не нужна (тесты) — используй
    /// `new_ephemeral`.
    pub fn new(data_dir: PathBuf) -> Arc<Self> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let session = match load_state(&data_dir) {
            Some(persisted) => {
                eprintln!(
                    "[pomodoro] restoring persisted state: phase={:?} is_running={} is_paused={}",
                    persisted.phase, persisted.is_running, persisted.is_paused
                );
                Session::from_persisted(clock, persisted)
            }
            None => Session::new(clock),
        };
        Self::assemble(session, Some(data_dir))
    }

    /// In-memory hosting без файла. Использовать только для тестов и
    /// сценариев где state не должен переживать перезапуск.
    pub fn new_ephemeral() -> Arc<Self> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        Self::assemble(Session::new(clock), None)
    }

    fn assemble(session: Session, data_dir: Option<PathBuf>) -> Arc<Self> {
        let session_events = session.subscribe();
        let session_arc = Arc::new(Mutex::new(session));

        let (events_tx, _) = broadcast::channel::<Value>(256);

        // Spawn: forward Session events → wire-formatted broadcast. Также
        // дёргает save_state() на phase_changed event'ах — это покрывает
        // авто-advance внутри tick() (work → break boundary).
        let evt_tx_clone = events_tx.clone();
        let session_for_evt = session_arc.clone();
        let data_dir_for_evt = data_dir.clone();
        tokio::spawn(forward_session_events(
            session_events,
            evt_tx_clone,
            session_for_evt,
            data_dir_for_evt,
        ));

        // Spawn: 1Hz ticker.
        let session_for_tick = session_arc.clone();
        tokio::spawn(ticker_loop(session_for_tick));

        Arc::new(Self {
            session: session_arc,
            events_tx,
            data_dir,
        })
    }

    async fn persist_locked(&self, session: &Session) {
        let Some(data_dir) = self.data_dir.as_ref() else {
            return;
        };
        persist_snapshot(data_dir, &session.snapshot_persisted());
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events_tx.subscribe()
    }

    /// Snapshot state, готовый для embed в WS response.
    pub async fn state_value(&self) -> Value {
        let s = self.session.lock().await;
        state_to_value(&s.snapshot())
    }

    // start: emits PhaseChanged → persist идёт через forward_session_events.
    pub async fn start(&self, config: SessionConfig) -> Value {
        let mut s = self.session.lock().await;
        s.start(config);
        state_to_value(&s.snapshot())
    }

    pub async fn pause(&self) -> Value {
        let mut s = self.session.lock().await;
        s.pause();
        let v = state_to_value(&s.snapshot());
        // pause() не шлёт событий — persist'им явно.
        self.persist_locked(&s).await;
        v
    }

    pub async fn resume(&self) -> Value {
        let mut s = self.session.lock().await;
        s.resume();
        let v = state_to_value(&s.snapshot());
        // resume() не шлёт событий — persist'им явно.
        self.persist_locked(&s).await;
        v
    }

    // skip: emits Finished + PhaseChanged (running) или PhaseChanged (non-idle non-running)
    // или ничего (idle no-op) — persist идёт через forward_session_events.
    pub async fn skip(&self) -> Value {
        let mut s = self.session.lock().await;
        s.skip();
        state_to_value(&s.snapshot())
    }

    // stop: emits PhaseChanged → persist (с удалением файла на idle) через forward_session_events.
    pub async fn stop(&self) -> Value {
        let mut s = self.session.lock().await;
        s.stop();
        state_to_value(&s.snapshot())
    }
}

/// Атомарная запись `<data_dir>/pomodoro-state.json` через temp + rename.
fn save_state(data_dir: &Path, snapshot: &PersistedSession) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let target = data_dir.join(STATE_FILE_BASENAME);
    let bytes = serde_json::to_vec(snapshot)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let temp_name = format!(".{STATE_FILE_BASENAME}.tmp.{}", std::process::id());
    let temp_path = data_dir.join(temp_name);
    {
        use std::io::Write as _;
        let mut f = std::fs::File::create(&temp_path)?;
        f.write_all(&bytes)?;
    }
    std::fs::rename(&temp_path, &target)?;
    Ok(())
}

/// Общая логика «idle+default → remove файл, иначе → save». Используется
/// и из ops (persist_locked), и из event-driven (persist_after_event).
fn persist_snapshot(data_dir: &Path, snapshot: &PersistedSession) {
    if snapshot.phase == Phase::Idle
        && !snapshot.is_running
        && !snapshot.is_paused
        && snapshot.completed_pomodoros == 0
    {
        let path = data_dir.join(STATE_FILE_BASENAME);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => eprintln!("[pomodoro] WARN failed to remove state file {path:?}: {e}"),
        }
        return;
    }
    if let Err(e) = save_state(data_dir, snapshot) {
        eprintln!("[pomodoro] WARN save_state failed: {e}");
    }
}

/// Читает persisted state. Возвращает `None` если файл отсутствует, не
/// парсится, или `version != CURRENT_VERSION` (логирует warning).
fn load_state(data_dir: &Path) -> Option<PersistedSession> {
    let path = data_dir.join(STATE_FILE_BASENAME);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            eprintln!("[pomodoro] WARN load_state read failed for {path:?}: {e}");
            return None;
        }
    };
    let parsed: PersistedSession = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[pomodoro] WARN load_state JSON parse failed for {path:?}: {e}");
            return None;
        }
    };
    if parsed.version != PersistedSession::CURRENT_VERSION {
        eprintln!(
            "[pomodoro] WARN load_state version mismatch (file={}, expected={}). Ignoring.",
            parsed.version,
            PersistedSession::CURRENT_VERSION
        );
        return None;
    }
    Some(parsed)
}

fn state_to_value(s: &ark_core::pomodoro::SessionState) -> Value {
    json!({
        "phase": phase_to_str(s.phase),
        "remainingMs": s.remaining_ms,
        "totalMs": s.total_ms,
        "completedPomodoros": s.completed_pomodoros,
        "isRunning": s.is_running,
        "isPaused": s.is_paused,
        "title": s.title,
        "tasks": s.tasks,
        // null когда idle/paused — renderer падает на server-provided remainingMs.
        "phaseEndsAtMs": s.phase_ends_at_ms,
    })
}

async fn forward_session_events(
    mut rx: broadcast::Receiver<SessionEvent>,
    out: broadcast::Sender<Value>,
    session: Arc<Mutex<Session>>,
    data_dir: Option<PathBuf>,
) {
    loop {
        match rx.recv().await {
            Ok(SessionEvent::Tick { state }) => {
                let mut v = json!({ "event": "pomodoro_tick" });
                merge_state(&mut v, &state);
                let _ = out.send(v);
                // tick'и не пишем — каждую секунду слишком часто.
            }
            Ok(SessionEvent::PhaseChanged { from, to, state }) => {
                let mut v = json!({
                    "event": "pomodoro_phase_changed",
                    "from": phase_to_str(from),
                    "to": phase_to_str(to),
                });
                merge_state(&mut v, &state);
                let _ = out.send(v);
                persist_after_event(&session, data_dir.as_deref()).await;
            }
            Ok(SessionEvent::Finished { finished, state }) => {
                let mut v = json!({
                    "event": "pomodoro_finished",
                    "finished": phase_to_str(finished),
                });
                merge_state(&mut v, &state);
                let _ = out.send(v);
                persist_after_event(&session, data_dir.as_deref()).await;
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("[pomodoro] WARN events lagged by {n}, persist may have been missed");
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

async fn persist_after_event(session: &Arc<Mutex<Session>>, data_dir: Option<&Path>) {
    let Some(data_dir) = data_dir else { return };
    let snapshot = {
        let s = session.lock().await;
        s.snapshot_persisted()
    };
    persist_snapshot(data_dir, &snapshot);
}

fn merge_state(v: &mut Value, state: &ark_core::pomodoro::SessionState) {
    let st = state_to_value(state);
    if let (Some(obj), Some(st_obj)) = (v.as_object_mut(), st.as_object()) {
        for (k, val) in st_obj {
            obj.insert(k.clone(), val.clone());
        }
    }
}

async fn ticker_loop(session: Arc<Mutex<Session>>) {
    // 1 Hz — keep-alive ticker. UI smoothness даёт renderer'овская
    // интерполяция по `phaseEndsAtMs`; backend нужен лишь для:
    //   * consistency `is_running` / `completed_pomodoros` после phase boundary,
    //   * delivery `pomodoro_phase_changed` / `pomodoro_finished` events,
    //   * sync state когда renderer закрыт.
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let mut s = session.lock().await;
        s.tick();
    }
}

/// Локальный response от `pomodoro.*` обработчика. Используется ws_server.
pub struct PomodoroResponse {
    pub ok: bool,
    pub data: Value,
    pub error: Option<String>,
}

impl PomodoroResponse {
    fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: Value::Null,
            error: Some(msg.into()),
        }
    }
}

/// Диспатч `pomodoro.<subop>` — обрабатывает start/pause/resume/skip/stop/get_state.
pub async fn handle_pomodoro_op(
    subop: &str,
    params: Value,
    host: &PomodoroHost,
) -> PomodoroResponse {
    match subop {
        "start" => {
            let cfg_value = params.get("config").cloned().unwrap_or(Value::Null);
            let cfg: SessionConfig = match serde_json::from_value(cfg_value) {
                Ok(c) => c,
                Err(e) => {
                    return PomodoroResponse::err(format!(
                        "pomodoro.start: invalid 'config': {e}"
                    ));
                }
            };
            let state = host.start(cfg).await;
            PomodoroResponse::ok(state)
        }
        "pause" => PomodoroResponse::ok(host.pause().await),
        "resume" => PomodoroResponse::ok(host.resume().await),
        "skip" => PomodoroResponse::ok(host.skip().await),
        "stop" => PomodoroResponse::ok(host.stop().await),
        "get_state" => PomodoroResponse::ok(host.state_value().await),
        other => PomodoroResponse::err(format!("pomodoro.{other}: unknown sub-operation")),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_returns_running_work_state() {
        let host = PomodoroHost::new_ephemeral();
        let params = json!({
            "config": {
                "workMin": 25,
                "shortBreakMin": 5,
                "longBreakMin": 15,
                "pomodorosUntilLongBreak": 4,
                "title": "Focus",
                "tasks": []
            }
        });
        let resp = handle_pomodoro_op("start", params, &host).await;
        assert!(resp.ok, "start failed: {:?}", resp.error);
        assert_eq!(resp.data["phase"], "work");
        assert_eq!(resp.data["isRunning"], true);
        assert_eq!(resp.data["totalMs"], 25 * 60_000_u64);
    }

    #[tokio::test]
    async fn get_state_idle_initially() {
        let host = PomodoroHost::new_ephemeral();
        let resp = handle_pomodoro_op("get_state", Value::Null, &host).await;
        assert!(resp.ok);
        assert_eq!(resp.data["phase"], "idle");
        assert_eq!(resp.data["isRunning"], false);
    }

    #[tokio::test]
    async fn stop_resets_to_idle() {
        let host = PomodoroHost::new_ephemeral();
        let params = json!({
            "config": {
                "workMin": 25,
                "shortBreakMin": 5,
                "longBreakMin": 15,
                "pomodorosUntilLongBreak": 4,
                "title": "Focus",
                "tasks": []
            }
        });
        handle_pomodoro_op("start", params, &host).await;
        let resp = handle_pomodoro_op("stop", Value::Null, &host).await;
        assert!(resp.ok);
        assert_eq!(resp.data["phase"], "idle");
    }

    #[tokio::test]
    async fn unknown_subop_errors() {
        let host = PomodoroHost::new_ephemeral();
        let resp = handle_pomodoro_op("explode", Value::Null, &host).await;
        assert!(!resp.ok);
        assert!(resp.error.unwrap().contains("unknown"));
    }

    #[tokio::test]
    async fn start_emits_phase_changed_event() {
        let host = PomodoroHost::new_ephemeral();
        let mut rx = host.subscribe();
        let params = json!({
            "config": {
                "workMin": 25,
                "shortBreakMin": 5,
                "longBreakMin": 15,
                "pomodorosUntilLongBreak": 4,
                "title": "Focus",
                "tasks": []
            }
        });
        handle_pomodoro_op("start", params, &host).await;
        // forward_session_events task должна получить событие и зашьорить его в out broadcast.
        // Даём task'у шанс выполниться.
        let evt = tokio::time::timeout(Duration::from_millis(500), rx.recv())
            .await
            .expect("event not received within 500ms")
            .expect("recv ok");
        assert_eq!(evt["event"], "pomodoro_phase_changed");
        assert_eq!(evt["from"], "idle");
        assert_eq!(evt["to"], "work");
    }

    // -----------------------------------------------------------------------
    // Persistence tests
    // -----------------------------------------------------------------------

    fn tmp_data_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "kosmos-pomodoro-test-{}-{}-{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tmp_data_dir("rt");
        let snapshot = PersistedSession {
            version: PersistedSession::CURRENT_VERSION,
            phase: Phase::Work,
            remaining_ms: 1_500_000,
            total_ms: 1_500_000,
            completed_pomodoros: 2,
            is_running: true,
            is_paused: false,
            phase_ends_at_ms: 9_999_999_999,
            last_config: Some(SessionConfig::default()),
        };
        save_state(&dir, &snapshot).expect("save_state should succeed");
        let loaded = load_state(&dir).expect("load_state should return Some");
        assert_eq!(loaded.phase, Phase::Work);
        assert_eq!(loaded.completed_pomodoros, 2);
        assert!(loaded.is_running);
        assert_eq!(loaded.phase_ends_at_ms, 9_999_999_999);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_state_missing_file_returns_none() {
        let dir = tmp_data_dir("missing");
        assert!(load_state(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_state_malformed_returns_none() {
        let dir = tmp_data_dir("malformed");
        std::fs::write(dir.join(STATE_FILE_BASENAME), b"{not json").unwrap();
        assert!(load_state(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_state_wrong_version_returns_none() {
        let dir = tmp_data_dir("wrong-ver");
        let bogus = serde_json::json!({
            "version": 999,
            "phase": "work",
            "remainingMs": 0u64,
            "totalMs": 0u64,
            "completedPomodoros": 0u32,
            "isRunning": false,
            "isPaused": false,
            "phaseEndsAtMs": 0u64,
            "lastConfig": null,
        });
        std::fs::write(
            dir.join(STATE_FILE_BASENAME),
            serde_json::to_vec(&bogus).unwrap(),
        )
        .unwrap();
        assert!(load_state(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn host_persists_state_on_start_and_restores_on_new() {
        let dir = tmp_data_dir("host-restart");
        let host = PomodoroHost::new(dir.clone());
        let params = json!({
            "config": {
                "workMin": 25,
                "shortBreakMin": 5,
                "longBreakMin": 15,
                "pomodorosUntilLongBreak": 4,
                "title": "Focus",
                "tasks": []
            }
        });
        let resp = handle_pomodoro_op("start", params, &host).await;
        assert!(resp.ok);
        // Persist теперь идёт через event path (forward_session_events task) —
        // ждём, пока taskk обработает PhaseChanged.
        tokio::time::sleep(Duration::from_millis(50)).await;
        // Файл должен существовать после start.
        let state_path = dir.join(STATE_FILE_BASENAME);
        assert!(state_path.exists(), "state file should exist after start");

        // Симулируем перезапуск: создаём новый host над тем же dir.
        let host2 = PomodoroHost::new(dir.clone());
        let resp2 = handle_pomodoro_op("get_state", Value::Null, &host2).await;
        assert!(resp2.ok);
        // Если стартовали 25-минутную work-фазу — она ещё активна.
        assert_eq!(resp2.data["phase"], "work");
        assert_eq!(resp2.data["isRunning"], true);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn host_removes_state_file_on_stop() {
        let dir = tmp_data_dir("host-stop");
        let host = PomodoroHost::new(dir.clone());
        let params = json!({
            "config": {
                "workMin": 25,
                "shortBreakMin": 5,
                "longBreakMin": 15,
                "pomodorosUntilLongBreak": 4,
                "title": "Focus",
                "tasks": []
            }
        });
        handle_pomodoro_op("start", params, &host).await;
        // Ждём persist через event path после start.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let state_path = dir.join(STATE_FILE_BASENAME);
        assert!(state_path.exists());

        handle_pomodoro_op("stop", Value::Null, &host).await;
        // Ждём persist через event path после stop (на idle файл удаляется).
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !state_path.exists(),
            "state file should be removed after stop"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
