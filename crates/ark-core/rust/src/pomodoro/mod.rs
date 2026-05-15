//! Pomodoro session state machine — pure logic, без I/O и без DB.
//!
//! Port of `extensions/horologion/src/lib/usePomodoro.ts` для Wave 2:
//! state machine жить должна в backend (kepler-backend), чтобы переживать
//! renderer crash / reload. Side effects (time_entry CRUD, audio, system
//! notifications) — остаются в renderer'е (listen на phase_changed event).
//!
//! Parity-tested: тот же набор сценариев что и в TS golden tests
//! (`extensions/horologion/tests/usePomodoro.test.ts`).
//!
//! Clock инжектится через trait `Clock`, чтобы tests могли использовать
//! MockClock с детерминированной advance().

pub mod clock;
pub mod events;
pub mod session;

pub use clock::{Clock, MockClock, SystemClock};
pub use events::SessionEvent;
pub use session::{Phase, Session, SessionConfig, SessionState, TaskRef};
