//! State-machine events delivered by the Cortex host over WebSocket.

use serde::Serialize;

use super::session::{Phase, SessionState};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEvent {
    Tick {
        state: SessionState,
    },
    PhaseChanged {
        from: Phase,
        to: Phase,
        state: SessionState,
    },
    Finished {
        finished: Phase,
        state: SessionState,
    },
}
