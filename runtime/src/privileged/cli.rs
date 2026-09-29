//! `kepler-backend privileged <subcommand>` — the single exe carrying all
//! privileged modes:
//!
//!   privileged install      — elevated one-time setup (copies the exe to
//!                             %ProgramFiles%\<Brand>\Service, registers and
//!                             starts the SCM service). Triggered via the
//!                             `runas` ShellExecute verb — one UAC prompt.
//!   privileged uninstall    — elevated teardown (stop/delete service,
//!                             remove the service copy).
//!   privileged status       — user-mode JSON status.
//!   privileged run-service  — SCM entry point (never launched by hand).
//!
//! Output is a single JSON `Outcome` line so calling scripts can inspect it.

use std::process::ExitCode;

use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct Outcome {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub needs_elevation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_dir: Option<String>,
}

impl Outcome {
    pub fn print(&self) -> ExitCode {
        let s = serde_json::to_string(self)
            .unwrap_or_else(|_| String::from(r#"{"ok":false,"error":"serialize failed"}"#));
        println!("{s}");
        ExitCode::from(if self.ok { 0 } else { 1 })
    }
}

/// Called first from `main` — when argv starts with `privileged` the whole
/// process is dedicated to the subcommand and never starts the Engine.
pub fn run_if_privileged(args: &[String]) -> Option<ExitCode> {
    if args.first().map(String::as_str) != Some("privileged") {
        return None;
    }
    Some(dispatch(&args[1..]))
}

#[cfg(windows)]
fn dispatch(args: &[String]) -> ExitCode {
    use crate::privileged::{scm, scm_status, service};
    match args.first().map(String::as_str) {
        // `install --grant-sid <sid>` — the SID of the *unelevated* user the
        // Engine runs as. Never derived from the elevated token (OTS UAC
        // would produce the admin's SID and lock the user out of the pipe).
        Some("install") => {
            scm::install(&flag_value(args, "--grant-sid").unwrap_or_default()).print()
        }
        Some("uninstall") => scm::uninstall().print(),
        Some("status") => scm_status::status_cli().print(),
        Some("run-service") => {
            let sid = flag_value(args, "--grant-sid").unwrap_or_default();
            service::run_service_entry(sid)
        }
        _ => {
            println!("usage: kepler-backend privileged <install|uninstall|status|run-service>");
            ExitCode::from(2)
        }
    }
}

#[cfg(windows)]
pub(crate) fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

#[cfg(not(windows))]
fn dispatch(_args: &[String]) -> ExitCode {
    println!(r#"{{"ok":false,"error":"privileged operations are Windows-only"}}"#);
    ExitCode::from(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_privileged_argv_is_not_intercepted() {
        assert!(run_if_privileged(&[]).is_none());
        assert!(run_if_privileged(&["--start".to_string()]).is_none());
        assert!(run_if_privileged(&["--core-worker".to_string()]).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn grant_sid_flag_parsing() {
        let args = vec![
            "install".into(),
            "--grant-sid".into(),
            "S-1-5-21-1-2-3-1001".into(),
        ];
        assert_eq!(
            flag_value(&args, "--grant-sid").as_deref(),
            Some("S-1-5-21-1-2-3-1001")
        );
        assert_eq!(flag_value(&args, "--missing"), None);
        assert_eq!(
            flag_value(&["install".into(), "--grant-sid".into()], "--grant-sid"),
            None
        );
    }

    /// `install` without/invalid `--grant-sid` fails before touching the SCM.
    #[cfg(windows)]
    #[test]
    fn install_rejects_bad_grant_sid_without_elevation() {
        for bad in ["", "garbage", "S-1-5-21-1;)(A;;GA;;;WD", "S-1-5-18 "] {
            let out = crate::privileged::scm::install(bad);
            assert!(!out.ok, "accepted {bad:?}");
            assert!(out.error.unwrap_or_default().contains("--grant-sid"));
        }
    }
}
