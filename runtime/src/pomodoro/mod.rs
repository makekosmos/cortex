//! Product-owned Pomodoro session state machine.
//!
//! The host, persistence, and WebSocket delivery live in `pomodoro_host`.

pub mod clock;
pub mod events;
pub mod session;

pub use clock::{Clock, MockClock, SystemClock};
pub use events::SessionEvent;
pub use session::{PersistedSession, Phase, Session, SessionConfig, SessionState, TaskRef};
