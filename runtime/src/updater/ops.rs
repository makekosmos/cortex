//! Dispatch for `updater.<subop>`, wired in
//! `crate::ws_server::dispatch_standard`. Shape mirrors the other per-host
//! dispatch modules in this crate (`pomodoro_host::handle_pomodoro_op`,
//! `dictation`'s `DictationResponse`) — a small local response struct rather
//! than reusing `ws_server::ops::LocalResponse`, which is `pub(super)` to
//! `ws_server` and not visible here.
//!
//! Every op here is reachable only over the Engine's authenticated
//! WebSocket (Manager/Host), never through the app-facing HTTP RPC path —
//! see `mod.rs`'s doc comment for why that boundary holds without an
//! explicit allow/deny check in this module.
use std::sync::Arc;

use serde_json::Value;

use super::UpdaterService;

pub(crate) struct UpdaterResponse {
    pub(crate) ok: bool,
    pub(crate) data: Value,
    pub(crate) error: Option<String>,
}

impl UpdaterResponse {
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

/// `updater.status` — current state, poll-friendly (no side effects).
/// `updater.check` — manual feed check; Engine also runs it on its own
///   shortly after startup and then periodically
///   (`UpdaterService::run_check_loop`). Finding a newer
///   version starts its download automatically (autoDownload=true parity
///   with `autoupdater-host.ts`).
/// `updater.download` — explicit (re)start of the pending download; a
///   no-op if one is already running or finished.
/// `updater.install` — once `status().state == "downloaded"`, applies the
///   payload: Windows launches the NSIS installer silently (`/S`, detached),
///   macOS mounts the DMG and hands a detached helper the bundle swap +
///   relaunch (`updater::macos`). Both paths surface failures as the `error`
///   state.
pub(crate) async fn handle_updater_op(
    subop: &str,
    _params: Value,
    service: &Arc<UpdaterService>,
) -> UpdaterResponse {
    match subop {
        "status" => UpdaterResponse::ok(service.status()),
        "check" => UpdaterResponse::ok(service.check().await),
        "download" => UpdaterResponse::ok(service.download()),
        "install" => UpdaterResponse::ok(service.install()),
        other => UpdaterResponse::err(format!("updater.{other}: unknown sub-operation")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn status_is_idle_before_any_check() {
        let dir = tempfile::tempdir().unwrap();
        let service = UpdaterService::new(dir.path().to_path_buf());
        let response = handle_updater_op("status", Value::Null, &service).await;
        assert!(response.ok);
        assert_eq!(response.data["state"], "idle");
    }

    #[tokio::test]
    async fn unknown_subop_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let service = UpdaterService::new(dir.path().to_path_buf());
        let response = handle_updater_op("frobnicate", Value::Null, &service).await;
        assert!(!response.ok);
        assert_eq!(
            response.error.as_deref(),
            Some("updater.frobnicate: unknown sub-operation")
        );
    }

    #[tokio::test]
    async fn download_before_check_reports_no_update_available() {
        let dir = tempfile::tempdir().unwrap();
        let service = UpdaterService::new(dir.path().to_path_buf());
        let response = handle_updater_op("download", Value::Null, &service).await;
        assert!(response.ok); // op dispatched fine; the *update* state is an error
        assert_eq!(response.data["state"], "error");
    }
}
