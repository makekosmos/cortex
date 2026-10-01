//! Pruning of `<engine_root>/versions/<v>` install dirs (KOS-261) — the rule
//! itself lives in [`prune`]; this module only hosts the two runners that
//! share it:
//!
//!   * `mundus-engine prune-versions` — one-shot CLI mode that
//!     `desktop/build/install-engine.ps1` invokes on the just-installed exe
//!     after switching `current.json`, so the selection rule is not
//!     duplicated in PowerShell;
//!   * [`prune_self_install`] — the startup pass `setup()` runs once the
//!     singleton is held, cleaning what existing users already accumulated.
//!
//! Both derive the engine root from the running exe's own path
//! (`<root>/versions/<v>/mundus-engine.exe`), so a dev build launched from
//! `target/` can never prune anything.

mod prune;

pub use prune::{engine_root_of_exe, Report};

use std::process::ExitCode;

/// Called first from `main` — when argv starts with `prune-versions` the
/// whole process is dedicated to the cleanup and never starts the Engine
/// (same shape as `privileged::cli::run_if_privileged`). Prints one JSON
/// outcome line for the calling installer.
pub fn run_if_prune_versions(args: &[String]) -> Option<ExitCode> {
    if args.first().map(String::as_str) != Some("prune-versions") {
        return None;
    }
    let outcome = std::env::current_exe()
        .map_err(|e| e.to_string())
        .and_then(|exe| {
            engine_root_of_exe(&exe)
                .ok_or_else(|| format!("{exe:?} is not installed under <root>/versions/<v>/"))
        })
        .and_then(|root| prune::prune(&root));
    Some(match outcome {
        Ok(report) => {
            let body = serde_json::json!({ "ok": true, "report": report });
            println!("{body}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", serde_json::json!({ "ok": false, "error": error }));
            ExitCode::FAILURE
        }
    })
}

/// The startup pass: prune the install this exe belongs to. No-op (returns
/// None) for dev builds and whenever the pointer says nothing may be
/// deleted; errors are logged, never fatal — the next start retries.
pub fn prune_self_install() -> Option<Report> {
    let exe = std::env::current_exe().ok()?;
    let root = engine_root_of_exe(&exe)?;
    match prune::prune(&root) {
        Ok(report) => report,
        Err(error) => {
            tracing::warn!(error = %error, "engine versions prune failed");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_prune_argv_is_not_intercepted() {
        assert!(run_if_prune_versions(&[]).is_none());
        assert!(run_if_prune_versions(&["--start".to_string()]).is_none());
        assert!(run_if_prune_versions(&["--core-worker".to_string()]).is_none());
    }
}
