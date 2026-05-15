//! Pomodoro Session — state machine c parity к TS `usePomodoro`.
//!
//! ## Контракт TS-parity
//!
//! * `start(config)`: idle → work (или paused → resume current phase).
//!   Сохраняет config в `last_context` для последующих transition'ов.
//! * `pause()` / `resume()`: замораживает / разморозкa remaining_ms.
//!   В TS используется `phaseEndsAt = now + remainingMs` при resume —
//!   мы делаем то же.
//! * `tick()`: вызывается ticker'ом каждую секунду. Если remaining_ms
//!   дошёл до 0 — finish_phase(): increment completed (если work
//!   завершился), переключение phase, опционально auto-start.
//! * `skip()`: иммедиатное завершение текущей фазы (как tick в момент
//!   remaining=0). Если phase==idle — no-op.
//! * `stop()`: жёсткий reset — idle, completed=0.
//!
//! ## События
//!
//! Любой mutation шлёт events через broadcast channel. Wrapper'ы в
//! kepler-backend subscribe'аются и forward'ят через WS клиентам.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use super::clock::Clock;
use super::events::SessionEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Work,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRef {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(rename = "workMin")]
    pub work_min: u32,
    #[serde(rename = "shortBreakMin")]
    pub short_break_min: u32,
    #[serde(rename = "longBreakMin")]
    pub long_break_min: u32,
    #[serde(rename = "pomodorosUntilLongBreak")]
    pub pomodoros_until_long_break: u32,
    #[serde(rename = "autoStartWork", default)]
    pub auto_start_work: bool,
    #[serde(rename = "autoStartBreak", default)]
    pub auto_start_break: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub tasks: Vec<TaskRef>,
    /// Optional override для work-фазы — используется command bus'ом
    /// (`horologion:pomodoro:25` / `:50`).
    #[serde(rename = "workMinOverride", default, skip_serializing_if = "Option::is_none")]
    pub work_min_override: Option<u32>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            work_min: 25,
            short_break_min: 5,
            long_break_min: 15,
            pomodoros_until_long_break: 4,
            auto_start_work: false,
            auto_start_break: false,
            title: String::new(),
            tasks: Vec::new(),
            work_min_override: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub phase: Phase,
    pub remaining_ms: u64,
    pub total_ms: u64,
    pub completed_pomodoros: u32,
    pub is_running: bool,
    pub is_paused: bool,
    /// Title последнего start'а — renderer использует для time_entry.
    pub title: String,
    /// Tasks — для multi-task split renderer'ом.
    pub tasks: Vec<TaskRef>,
}

impl SessionState {
    pub fn idle() -> Self {
        Self {
            phase: Phase::Idle,
            remaining_ms: 0,
            total_ms: 0,
            completed_pomodoros: 0,
            is_running: false,
            is_paused: false,
            title: String::new(),
            tasks: Vec::new(),
        }
    }
}

pub struct Session {
    clock: Arc<dyn Clock>,
    events_tx: broadcast::Sender<SessionEvent>,
    // Mutable state.
    phase: Phase,
    remaining_ms: u64,
    total_ms: u64,
    completed_pomodoros: u32,
    is_running: bool,
    is_paused: bool,
    /// При `is_running && !is_paused` — wall-clock момент в ms когда
    /// текущая фаза закончится. TS-impl: `phaseEndsAt`.
    phase_ends_at_ms: u64,
    last_config: Option<SessionConfig>,
}

impl Session {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        let (events_tx, _) = broadcast::channel::<SessionEvent>(128);
        Self {
            clock,
            events_tx,
            phase: Phase::Idle,
            remaining_ms: 0,
            total_ms: 0,
            completed_pomodoros: 0,
            is_running: false,
            is_paused: false,
            phase_ends_at_ms: 0,
            last_config: None,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.events_tx.subscribe()
    }

    pub fn snapshot(&self) -> SessionState {
        let (title, tasks) = match &self.last_config {
            Some(c) => (c.title.clone(), c.tasks.clone()),
            None => (String::new(), Vec::new()),
        };
        SessionState {
            phase: self.phase,
            remaining_ms: self.remaining_ms,
            total_ms: self.total_ms,
            completed_pomodoros: self.completed_pomodoros,
            is_running: self.is_running,
            is_paused: self.is_paused,
            title,
            tasks,
        }
    }

    fn duration_ms_for_phase(&self, p: Phase, cfg: &SessionConfig) -> u64 {
        match p {
            Phase::Idle => 0,
            Phase::Work => {
                let min = cfg.work_min_override.unwrap_or(cfg.work_min);
                (min as u64) * 60_000
            }
            Phase::ShortBreak => (cfg.short_break_min as u64) * 60_000,
            Phase::LongBreak => (cfg.long_break_min as u64) * 60_000,
        }
    }

    fn next_phase_after(&self, p: Phase, cfg: &SessionConfig) -> Phase {
        match p {
            Phase::Work => {
                if self.completed_pomodoros + 1 >= cfg.pomodoros_until_long_break {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            }
            Phase::ShortBreak | Phase::LongBreak => Phase::Work,
            Phase::Idle => Phase::Work,
        }
    }

    /// Запуск фазы p с config cfg. Идемпотентно перезатирает state.
    fn start_phase(&mut self, p: Phase, cfg: SessionConfig) {
        let from = self.phase;
        let total = self.duration_ms_for_phase(p, &cfg);
        let now = self.clock.now_ms();
        self.phase = p;
        self.total_ms = total;
        self.remaining_ms = total;
        self.is_running = true;
        self.is_paused = false;
        self.phase_ends_at_ms = now.saturating_add(total);
        self.last_config = Some(cfg);
        let _ = self.events_tx.send(SessionEvent::PhaseChanged {
            from,
            to: p,
            state: self.snapshot(),
        });
    }

    /// `start(config)` — entry point из ARK op `pomodoro.start`. Если phase==idle
    /// → start work. Если paused → resume. Если уже running — re-start с тем же
    /// config (как TS — `start()` пере-входит в startPhase).
    pub fn start(&mut self, config: SessionConfig) {
        let target = if self.phase == Phase::Idle {
            Phase::Work
        } else {
            self.phase
        };
        self.start_phase(target, config);
    }

    pub fn pause(&mut self) {
        if !self.is_running || self.is_paused {
            return;
        }
        self.is_paused = true;
    }

    pub fn resume(&mut self) {
        if !self.is_paused {
            return;
        }
        self.is_paused = false;
        let now = self.clock.now_ms();
        self.phase_ends_at_ms = now.saturating_add(self.remaining_ms);
    }

    /// Tick — пересчитать remaining_ms. Если remaining=0 → finish_phase().
    pub fn tick(&mut self) {
        if !self.is_running || self.is_paused {
            return;
        }
        let now = self.clock.now_ms();
        self.remaining_ms = self.phase_ends_at_ms.saturating_sub(now);
        // Emit Tick event (subscribers могут throttle).
        let _ = self.events_tx.send(SessionEvent::Tick {
            state: self.snapshot(),
        });
        if self.remaining_ms == 0 {
            self.finish_phase();
        }
    }

    fn finish_phase(&mut self) {
        let finished = self.phase;
        self.is_running = false;
        self.is_paused = false;
        self.remaining_ms = 0;

        if finished == Phase::Work {
            self.completed_pomodoros += 1;
        }

        let cfg = self.last_config.clone().unwrap_or_default();
        let next = self.next_phase_after(finished, &cfg);

        let _ = self.events_tx.send(SessionEvent::Finished {
            finished,
            state: self.snapshot(),
        });

        let should_auto = match next {
            Phase::Work => cfg.auto_start_work,
            Phase::ShortBreak | Phase::LongBreak => cfg.auto_start_break,
            Phase::Idle => false,
        };

        if should_auto {
            self.start_phase(next, cfg);
        } else {
            // Просто переключаем phase + totalMs, не запускаем.
            let total = self.duration_ms_for_phase(next, &cfg);
            let from = self.phase;
            self.phase = next;
            self.total_ms = total;
            self.remaining_ms = total;
            let _ = self.events_tx.send(SessionEvent::PhaseChanged {
                from,
                to: next,
                state: self.snapshot(),
            });
        }
    }

    pub fn skip(&mut self) {
        // TS-parity: skip() во время idle — no-op.
        if self.phase == Phase::Idle && !self.is_running {
            return;
        }
        if self.is_running {
            self.remaining_ms = 0;
            self.finish_phase();
        } else {
            // Phase running == false, но не idle — это «застряли в next-фазе,
            // ждём ручного старта». TS: переключается на next-фазу.
            let cfg = self.last_config.clone().unwrap_or_default();
            let next = self.next_phase_after(self.phase, &cfg);
            let total = self.duration_ms_for_phase(next, &cfg);
            let from = self.phase;
            self.phase = next;
            self.total_ms = total;
            self.remaining_ms = total;
            let _ = self.events_tx.send(SessionEvent::PhaseChanged {
                from,
                to: next,
                state: self.snapshot(),
            });
        }
    }

    pub fn stop(&mut self) {
        let from = self.phase;
        self.is_running = false;
        self.is_paused = false;
        self.phase = Phase::Idle;
        self.remaining_ms = 0;
        self.total_ms = 0;
        self.completed_pomodoros = 0;
        let _ = self.events_tx.send(SessionEvent::PhaseChanged {
            from,
            to: Phase::Idle,
            state: self.snapshot(),
        });
    }
}

// ---------------------------------------------------------------------------
// Unit tests — parity к TS golden tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pomodoro::clock::MockClock;
    use std::sync::Arc;

    fn cfg_default() -> SessionConfig {
        SessionConfig::default()
    }

    fn cfg_auto_break() -> SessionConfig {
        SessionConfig {
            auto_start_break: true,
            ..SessionConfig::default()
        }
    }

    fn cfg_auto_all() -> SessionConfig {
        SessionConfig {
            auto_start_break: true,
            auto_start_work: true,
            ..SessionConfig::default()
        }
    }

    #[test]
    fn start_from_idle_enters_work() {
        let clock = Arc::new(MockClock::new(1_000_000));
        let mut s = Session::new(clock);
        s.start(cfg_default());
        let st = s.snapshot();
        assert_eq!(st.phase, Phase::Work);
        assert!(st.is_running);
        assert!(!st.is_paused);
        assert_eq!(st.total_ms, 25 * 60_000);
        assert_eq!(st.remaining_ms, 25 * 60_000);
    }

    #[test]
    fn tick_advances_remaining_ms() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        let before = s.snapshot().remaining_ms;
        clock.advance(5 * 60_000);
        s.tick();
        let after = s.snapshot().remaining_ms;
        assert!(after < before);
        assert_eq!(after, before - 5 * 60_000);
    }

    #[test]
    fn pause_freezes_remaining() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        clock.advance(60_000);
        s.tick();
        let frozen = s.snapshot().remaining_ms;
        s.pause();
        clock.advance(30_000);
        s.tick(); // должен быть no-op (paused)
        assert_eq!(s.snapshot().remaining_ms, frozen);
    }

    #[test]
    fn resume_continues_from_paused() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        clock.advance(60_000);
        s.tick();
        let frozen = s.snapshot().remaining_ms;
        s.pause();
        clock.advance(30_000); // в паузе
        s.resume();
        clock.advance(60_000);
        s.tick();
        assert_eq!(s.snapshot().remaining_ms, frozen - 60_000);
    }

    #[test]
    fn work_finishes_and_enters_short_break() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default()); // auto_start_break = false
        clock.advance(25 * 60_000);
        s.tick();
        let st = s.snapshot();
        assert_eq!(st.completed_pomodoros, 1);
        // auto_start_break=false → phase переключился но не запущен
        assert_eq!(st.phase, Phase::ShortBreak);
        assert!(!st.is_running);
    }

    #[test]
    fn four_pomodoros_then_long_break() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock);
        s.start(cfg_auto_all());
        for _ in 0..4 {
            // skip work → autoStartBreak → break
            s.skip();
            // skip break → autoStartWork → next work (после 4-го — нет, longBreak уже)
            if s.snapshot().phase != Phase::LongBreak {
                s.skip();
            }
        }
        let st = s.snapshot();
        assert_eq!(st.completed_pomodoros, 4);
        assert_eq!(st.phase, Phase::LongBreak);
    }

    #[test]
    fn skip_during_work_finishes_immediately() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        clock.advance(60_000);
        s.skip();
        let st = s.snapshot();
        assert_eq!(st.completed_pomodoros, 1);
        assert_eq!(st.phase, Phase::ShortBreak);
    }

    #[test]
    fn stop_resets_completed_counter() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_auto_all());
        s.skip(); // → break (completed=1)
        s.skip(); // → work
        s.skip(); // → break (completed=2)
        assert_eq!(s.snapshot().completed_pomodoros, 2);
        s.stop();
        let st = s.snapshot();
        assert_eq!(st.phase, Phase::Idle);
        assert_eq!(st.completed_pomodoros, 0);
        assert!(!st.is_running);
        assert_eq!(st.remaining_ms, 0);
    }

    #[test]
    fn auto_start_break_when_enabled() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_auto_break());
        clock.advance(25 * 60_000);
        s.tick();
        let st = s.snapshot();
        assert_eq!(st.phase, Phase::ShortBreak);
        assert!(st.is_running);
        assert_eq!(st.total_ms, 5 * 60_000);
    }

    #[test]
    fn work_min_override_applied() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock);
        let cfg = SessionConfig {
            work_min_override: Some(50),
            ..SessionConfig::default()
        };
        s.start(cfg);
        let st = s.snapshot();
        assert_eq!(st.total_ms, 50 * 60_000);
    }

    #[test]
    fn skip_on_idle_is_noop() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock);
        s.skip();
        let st = s.snapshot();
        assert_eq!(st.phase, Phase::Idle);
        assert!(!st.is_running);
    }

    #[test]
    fn subscribe_receives_phase_changed_on_start() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock);
        let mut rx = s.subscribe();
        s.start(cfg_default());
        let evt = rx.try_recv().expect("should receive event");
        match evt {
            SessionEvent::PhaseChanged { from, to, .. } => {
                assert_eq!(from, Phase::Idle);
                assert_eq!(to, Phase::Work);
            }
            other => panic!("expected PhaseChanged, got {other:?}"),
        }
    }
}
