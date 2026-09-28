//! Wire-facing status for `updater.status` — the Manager GPUI view polls
//! this shape directly. Mirrors the Electron `UpdateState` union
//! (`desktop/shared/ipc-types.ts`) closely enough to keep the Russian copy
//! in `useKeplerUpdate.ts` reusable, but flattened into one struct (the
//! Engine RPC boundary uses flat JSON objects, not tagged unions — see
//! `runtime/src/pomodoro_host.rs` wire-format note).
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Idle,
    Checking,
    Available,
    Downloading,
    Downloaded,
    NotAvailable,
    Error,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Checking => "checking",
            Phase::Available => "available",
            Phase::Downloading => "downloading",
            Phase::Downloaded => "downloaded",
            Phase::NotAvailable => "not-available",
            Phase::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UpdaterStatus {
    pub(crate) phase: Phase,
    pub(crate) current_version: String,
    pub(crate) new_version: Option<String>,
    pub(crate) percent: Option<u32>,
    pub(crate) message: Option<String>,
    pub(crate) checked_at_ms: Option<i64>,
}

impl UpdaterStatus {
    pub(crate) fn idle(current_version: String) -> Self {
        Self {
            phase: Phase::Idle,
            current_version,
            new_version: None,
            percent: None,
            message: None,
            checked_at_ms: None,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        let mut out = json!({
            "state": self.phase.as_str(),
            "currentVersion": self.current_version,
        });
        let map = out.as_object_mut().expect("object literal");
        if let Some(version) = &self.new_version {
            map.insert("newVersion".into(), Value::String(version.clone()));
        }
        if let Some(percent) = self.percent {
            map.insert("percent".into(), Value::Number(percent.into()));
        }
        if let Some(message) = &self.message {
            map.insert("message".into(), Value::String(message.clone()));
        }
        if let Some(checked_at) = self.checked_at_ms {
            map.insert("checkedAt".into(), Value::Number(checked_at.into()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_status_carries_only_current_version() {
        let status = UpdaterStatus::idle("0.5.3".into());
        assert_eq!(
            status.to_json(),
            json!({ "state": "idle", "currentVersion": "0.5.3" })
        );
    }

    #[test]
    fn downloading_status_includes_percent_and_new_version() {
        let status = UpdaterStatus {
            phase: Phase::Downloading,
            current_version: "0.5.3".into(),
            new_version: Some("0.5.4".into()),
            percent: Some(42),
            message: None,
            checked_at_ms: None,
        };
        assert_eq!(
            status.to_json(),
            json!({
                "state": "downloading",
                "currentVersion": "0.5.3",
                "newVersion": "0.5.4",
                "percent": 42,
            })
        );
    }

    #[test]
    fn error_status_includes_message() {
        let status = UpdaterStatus {
            phase: Phase::Error,
            current_version: "0.5.3".into(),
            new_version: None,
            percent: None,
            message: Some("unavailable".into()),
            checked_at_ms: None,
        };
        assert_eq!(status.to_json()["message"], "unavailable");
    }
}
