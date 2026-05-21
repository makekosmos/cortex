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
    /// Wallclock (Unix ms) когда текущая фаза закончится. `Some` только
    /// если `is_running && !is_paused` — renderer интерполирует
    /// `remainingMs = max(0, phase_ends_at_ms - Date.now())` локально
    /// (smooth 30fps), backend тикает 1 Hz только для consistency
    /// (is_running / completed_pomodoros / phase-boundary).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_ends_at_ms: Option<u64>,
}

/// Persisted snapshot — что мы пишем в `<data_dir>/pomodoro-state.json`.
/// Отдельная struct от `SessionState`, так как нужно сохранять `last_config`
/// (для advance логики на restore) + `phase_ends_at_ms` всегда (а не только
/// когда running) — иначе на restore из паузы нечего показать.
///
/// `version` — для будущих миграций. Сейчас всегда `1`. На load если version
/// не совпадает — host логирует warning и не реставрирует.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedSession {
    pub version: u32,
    pub phase: Phase,
    pub remaining_ms: u64,
    pub total_ms: u64,
    pub completed_pomodoros: u32,
    pub is_running: bool,
    pub is_paused: bool,
    /// Wallclock (Unix ms) когда running phase должна закончиться. Сохраняем
    /// всегда — на restore advance логика смотрит на него относительно now.
    pub phase_ends_at_ms: u64,
    pub last_config: Option<SessionConfig>,
}

impl PersistedSession {
    pub const CURRENT_VERSION: u32 = 1;
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
            phase_ends_at_ms: None,
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

    /// Snapshot для persistence на диск (см. `PersistedSession`).
    pub fn snapshot_persisted(&self) -> PersistedSession {
        PersistedSession {
            version: PersistedSession::CURRENT_VERSION,
            phase: self.phase,
            remaining_ms: self.remaining_ms,
            total_ms: self.total_ms,
            completed_pomodoros: self.completed_pomodoros,
            is_running: self.is_running,
            is_paused: self.is_paused,
            phase_ends_at_ms: self.phase_ends_at_ms,
            last_config: self.last_config.clone(),
        }
    }

    /// Восстановление Session из persisted state. Если был запущен и
    /// `phase_ends_at_ms <= now_ms` — фаза истекла во время downtime;
    /// эмулируем `finish_phase()` в loop (с limit 10) до тех пор, пока
    /// не окажемся в non-expired фазе или idle.
    ///
    /// На restore НЕ дёргаем `events_tx` для intermediate advance — клиенты
    /// ещё не подключены. Возвращаем готовый Session — caller (pomodoro_host)
    /// после connection может вручную выслать одно phase_changed для текущей
    /// фазы.
    pub fn from_persisted(clock: Arc<dyn Clock>, persisted: PersistedSession) -> Self {
        let (events_tx, _) = broadcast::channel::<SessionEvent>(128);
        let mut s = Self {
            clock,
            events_tx,
            phase: persisted.phase,
            remaining_ms: persisted.remaining_ms,
            total_ms: persisted.total_ms,
            completed_pomodoros: persisted.completed_pomodoros,
            is_running: persisted.is_running,
            is_paused: persisted.is_paused,
            phase_ends_at_ms: persisted.phase_ends_at_ms,
            last_config: persisted.last_config,
        };

        // Если paused — оставляем как есть; ничего не advance'им. На resume
        // anchor пересчитается.
        if s.is_paused {
            return s;
        }

        // Если running — advance через истёкшие фазы.
        let now = s.clock.now_ms();
        let mut iterations = 0;
        const MAX_ITER: u32 = 10;
        while s.is_running && !s.is_paused && s.phase_ends_at_ms <= now && iterations < MAX_ITER {
            iterations += 1;
            // Эмулируем `finish_phase()` но БЕЗ broadcast (никто не слушает).
            let cfg = s.last_config.clone().unwrap_or_default();
            let finished = s.phase;
            s.is_running = false;
            s.is_paused = false;
            s.remaining_ms = 0;
            if finished == Phase::Work {
                s.completed_pomodoros += 1;
            }
            let next = s.next_phase_after(finished, &cfg);
            let should_auto = match next {
                Phase::Work => cfg.auto_start_work,
                Phase::ShortBreak | Phase::LongBreak => cfg.auto_start_break,
                Phase::Idle => false,
            };
            if should_auto {
                // Re-enter target phase silently — повторяем body `start_phase`
                // без emit'а событий.
                let total = s.duration_ms_for_phase(next, &cfg);
                s.phase = next;
                s.total_ms = total;
                s.remaining_ms = total;
                s.is_running = true;
                s.is_paused = false;
                s.phase_ends_at_ms = now.saturating_add(total);
                s.last_config = Some(cfg);
                // Loop ещё раз — может новая фаза тоже истекла (edge: очень
                // длинный downtime + auto_start_*).
            } else {
                let total = s.duration_ms_for_phase(next, &cfg);
                s.phase = next;
                s.total_ms = total;
                s.remaining_ms = total;
                // is_running уже false — break.
                break;
            }
        }

        // Если ещё running после loop — пересчитать remaining_ms (как tick()
        // без emit события).
        if s.is_running && !s.is_paused {
            s.remaining_ms = s.phase_ends_at_ms.saturating_sub(now);
        }

        s
    }

    pub fn snapshot(&self) -> SessionState {
        let (title, tasks) = match &self.last_config {
            Some(c) => (c.title.clone(), c.tasks.clone()),
            None => (String::new(), Vec::new()),
        };
        let phase_ends_at_ms = if self.is_running && !self.is_paused {
            Some(self.phase_ends_at_ms)
        } else {
            None
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
            phase_ends_at_ms,
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

    // NOTE: emits no events — host must persist explicitly.
    pub fn pause(&mut self) {
        if !self.is_running || self.is_paused {
            return;
        }
        self.is_paused = true;
    }

    // NOTE: emits no events — host must persist explicitly.
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
    fn snapshot_phase_ends_at_ms_some_when_running() {
        let clock = Arc::new(MockClock::new(1_000_000));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        let st = s.snapshot();
        assert_eq!(
            st.phase_ends_at_ms,
            Some(1_000_000 + 25 * 60_000),
            "running session must expose wallclock phase_ends_at_ms"
        );
    }

    #[test]
    fn snapshot_phase_ends_at_ms_none_when_paused() {
        let clock = Arc::new(MockClock::new(1_000_000));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        clock.advance(60_000);
        s.tick();
        s.pause();
        let st = s.snapshot();
        assert!(
            st.phase_ends_at_ms.is_none(),
            "paused session must hide phase_ends_at_ms — renderer falls back на remaining_ms"
        );
        // remaining_ms всё ещё доступен.
        assert!(st.remaining_ms > 0);
    }

    #[test]
    fn snapshot_phase_ends_at_ms_none_when_idle() {
        let clock = Arc::new(MockClock::new(1_000_000));
        let s = Session::new(clock);
        let st = s.snapshot();
        assert!(st.phase_ends_at_ms.is_none());
    }

    #[test]
    fn snapshot_phase_ends_at_ms_updates_on_resume() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        clock.advance(60_000);
        s.tick();
        s.pause();
        // в паузе clock уходит вперёд, anchor должен пересчитаться на resume
        clock.advance(30_000);
        s.resume();
        let st = s.snapshot();
        // remaining_ms был 24*60_000, anchor = now (90_000) + remaining_ms
        assert_eq!(st.phase_ends_at_ms, Some(90_000 + 24 * 60_000));
    }

    #[test]
    fn persisted_roundtrip_preserves_running_state() {
        let clock = Arc::new(MockClock::new(1_000_000));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        let persisted = s.snapshot_persisted();
        assert_eq!(persisted.version, PersistedSession::CURRENT_VERSION);
        assert!(persisted.is_running);
        assert_eq!(persisted.phase, Phase::Work);

        // Reconstruct под тем же clock — phase_ends_at_ms ещё в будущем.
        let restored = Session::from_persisted(clock.clone(), persisted);
        let st = restored.snapshot();
        assert_eq!(st.phase, Phase::Work);
        assert!(st.is_running);
        assert!(!st.is_paused);
        assert_eq!(st.total_ms, 25 * 60_000);
        // remaining_ms могло измениться от tick'а на restore — но раз
        // clock не двигался, оно ≈ total_ms.
        assert_eq!(st.remaining_ms, 25 * 60_000);
    }

    #[test]
    fn persisted_restore_advances_through_expired_work_phase() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default()); // work, ends at 25*60_000
        let persisted = s.snapshot_persisted();

        // Эмулируем downtime: 30 минут прошло, work-фаза истекла.
        let later_clock = Arc::new(MockClock::new(30 * 60_000));
        let restored = Session::from_persisted(later_clock, persisted);
        let st = restored.snapshot();
        // auto_start_break=false → advance остановился на ShortBreak idle.
        assert_eq!(st.phase, Phase::ShortBreak);
        assert!(!st.is_running);
        assert_eq!(st.completed_pomodoros, 1);
    }

    #[test]
    fn persisted_restore_preserves_paused_state() {
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_default());
        clock.advance(60_000);
        s.tick();
        s.pause();
        let persisted = s.snapshot_persisted();
        assert!(persisted.is_paused);

        // Даже если clock уехал далеко в будущее — paused state не advance'ится.
        let later_clock = Arc::new(MockClock::new(10 * 60 * 60_000));
        let restored = Session::from_persisted(later_clock, persisted);
        let st = restored.snapshot();
        assert_eq!(st.phase, Phase::Work);
        assert!(st.is_paused);
        // remaining_ms сохранилось со времени паузы (24 * 60_000).
        assert!(st.remaining_ms > 0);
    }

    #[test]
    fn persisted_restore_idle_remains_idle() {
        let clock = Arc::new(MockClock::new(0));
        let s = Session::new(clock.clone());
        let persisted = s.snapshot_persisted();
        let restored = Session::from_persisted(clock, persisted);
        let st = restored.snapshot();
        assert_eq!(st.phase, Phase::Idle);
        assert!(!st.is_running);
    }

    #[test]
    fn persisted_restore_auto_chain_terminates_under_max_iter() {
        // auto_start_break + auto_start_work + долгий downtime → advance
        // должен остановиться на MAX_ITER даже если бесконечно бы цеплялся.
        let clock = Arc::new(MockClock::new(0));
        let mut s = Session::new(clock.clone());
        s.start(cfg_auto_all());
        let persisted = s.snapshot_persisted();

        // Огромный downtime — 24 часа.
        let later_clock = Arc::new(MockClock::new(24 * 60 * 60_000));
        let restored = Session::from_persisted(later_clock, persisted);
        // Просто проверяем что вернулись — не зациклились.
        let _st = restored.snapshot();
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
