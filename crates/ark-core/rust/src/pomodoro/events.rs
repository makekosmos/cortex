//! События state-machine — emitted из Session, потребляются wrapper'ом в
//! kepler-backend для broadcast через WS (`pomodoro_tick`, `pomodoro_phase_changed`,
//! `pomodoro_finished`).

use serde::Serialize;

use super::session::{Phase, SessionState};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEvent {
    /// Периодический тик (every 1 second в production). Renderer обновляет UI.
    Tick { state: SessionState },
    /// Фаза сменилась — work→shortBreak, shortBreak→work, etc. Renderer
    /// делает time_entry CRUD на этом событии.
    PhaseChanged {
        from: Phase,
        to: Phase,
        state: SessionState,
    },
    /// Текущая фаза завершилась (по timer'у или skip). Может сопровождаться
    /// PhaseChanged если auto-start enabled.
    Finished {
        finished: Phase,
        state: SessionState,
    },
}
