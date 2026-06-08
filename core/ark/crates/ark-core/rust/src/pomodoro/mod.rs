//! Pomodoro session state machine — pure logic, без I/O и без DB.
//!
//! Backend-owned state machine for focus sessions. It lives in kepler-backend
//! so active sessions survive renderer crash / reload.
//!
//! Covered by deterministic Rust tests around phase transitions and clock control.
//!
//! Clock инжектится через trait `Clock`, чтобы tests могли использовать
//! MockClock с детерминированной advance().

pub mod clock;
pub mod events;
pub mod session;

pub use clock::{Clock, MockClock, SystemClock};
pub use events::SessionEvent;
pub use session::{PersistedSession, Phase, Session, SessionConfig, SessionState, TaskRef};
