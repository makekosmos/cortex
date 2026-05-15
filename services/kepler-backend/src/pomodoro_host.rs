// Pomodoro host: singleton Session живёт в backend'е (Arc<Mutex<...>>) +
// tokio ticker который раз в секунду дёргает Session::tick(). События
// сериализуются и форвардятся всем connected WS-клиентам через broadcast.
//
// Wire format flat (см. запреты forbidden.md: никаких nested {kind,type,payload}):
//   {"event":"pomodoro_tick", "phase":"work", "remainingMs":..., ...state}
//   {"event":"pomodoro_phase_changed", "from":"work", "to":"shortBreak", ...state}
//   {"event":"pomodoro_finished", "finished":"work", ...state}
//
// ARK ops dispatched в ws_server (см. handle_pomodoro_op):
//   pomodoro.start { config }
//   pomodoro.pause
//   pomodoro.resume
//   pomodoro.skip
//   pomodoro.stop
//   pomodoro.get_state

use std::sync::Arc;
use std::time::Duration;

use ark_core::pomodoro::{Clock, Phase, Session, SessionConfig, SessionEvent, SystemClock};
use serde_json::{json, Value};
use tokio::sync::{broadcast, Mutex};

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
}

impl PomodoroHost {
    pub fn new() -> Arc<Self> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let session = Session::new(clock);
        let session_events = session.subscribe();
        let session_arc = Arc::new(Mutex::new(session));

        let (events_tx, _) = broadcast::channel::<Value>(256);

        // Spawn: forward Session events → wire-formatted broadcast.
        let evt_tx_clone = events_tx.clone();
        tokio::spawn(forward_session_events(session_events, evt_tx_clone));

        // Spawn: 1Hz ticker.
        let session_for_tick = session_arc.clone();
        tokio::spawn(ticker_loop(session_for_tick));

        Arc::new(Self {
            session: session_arc,
            events_tx,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events_tx.subscribe()
    }

    /// Snapshot state, готовый для embed в WS response.
    pub async fn state_value(&self) -> Value {
        let s = self.session.lock().await;
        state_to_value(&s.snapshot())
    }

    pub async fn start(&self, config: SessionConfig) -> Value {
        let mut s = self.session.lock().await;
        s.start(config);
        state_to_value(&s.snapshot())
    }

    pub async fn pause(&self) -> Value {
        let mut s = self.session.lock().await;
        s.pause();
        state_to_value(&s.snapshot())
    }

    pub async fn resume(&self) -> Value {
        let mut s = self.session.lock().await;
        s.resume();
        state_to_value(&s.snapshot())
    }

    pub async fn skip(&self) -> Value {
        let mut s = self.session.lock().await;
        s.skip();
        state_to_value(&s.snapshot())
    }

    pub async fn stop(&self) -> Value {
        let mut s = self.session.lock().await;
        s.stop();
        state_to_value(&s.snapshot())
    }
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
    })
}

async fn forward_session_events(
    mut rx: broadcast::Receiver<SessionEvent>,
    out: broadcast::Sender<Value>,
) {
    loop {
        match rx.recv().await {
            Ok(SessionEvent::Tick { state }) => {
                let mut v = json!({ "event": "pomodoro_tick" });
                merge_state(&mut v, &state);
                let _ = out.send(v);
            }
            Ok(SessionEvent::PhaseChanged { from, to, state }) => {
                let mut v = json!({
                    "event": "pomodoro_phase_changed",
                    "from": phase_to_str(from),
                    "to": phase_to_str(to),
                });
                merge_state(&mut v, &state);
                let _ = out.send(v);
            }
            Ok(SessionEvent::Finished { finished, state }) => {
                let mut v = json!({
                    "event": "pomodoro_finished",
                    "finished": phase_to_str(finished),
                });
                merge_state(&mut v, &state);
                let _ = out.send(v);
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
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
    // 250ms — parity с TS setInterval(..., 250) в legacy usePomodoro. Ниже —
    // overkill (ws traffic), выше — UI заметно лагает в e2e тестах которые
    // ожидают first-tick в 1500ms окне.
    let mut interval = tokio::time::interval(Duration::from_millis(250));
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
        let host = PomodoroHost::new();
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
        let host = PomodoroHost::new();
        let resp = handle_pomodoro_op("get_state", Value::Null, &host).await;
        assert!(resp.ok);
        assert_eq!(resp.data["phase"], "idle");
        assert_eq!(resp.data["isRunning"], false);
    }

    #[tokio::test]
    async fn stop_resets_to_idle() {
        let host = PomodoroHost::new();
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
        let host = PomodoroHost::new();
        let resp = handle_pomodoro_op("explode", Value::Null, &host).await;
        assert!(!resp.ok);
        assert!(resp.error.unwrap().contains("unknown"));
    }

    #[tokio::test]
    async fn start_emits_phase_changed_event() {
        let host = PomodoroHost::new();
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
}
