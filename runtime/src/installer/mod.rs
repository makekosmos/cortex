//! Installer subcommands of `mundus-engine` (KOS-306).
//!
//! The NSIS installer used to drive three PowerShell invocations
//! (`install-engine.ps1`, `engine-post-install.ps1 -MigrateAutostart`,
//! `-StartEngine`) plus fifteen `taskkill /F` spawns — exactly the feature
//! set Defender's first-sight ML associates with a dropper. That logic now
//! lives here, in-process, behind three one-shot subcommands dispatched
//! from `main` before normal startup (same shape as
//! `privileged::cli::run_if_privileged`):
//!
//!   mundus-engine install --manifest <path> --target-root <root>
//!       copy the staged payload into `<root>/versions/<v>` verifying
//!       manifest hashes, monotonic no-downgrade, legacy-registration
//!       takeover, autostart refresh, prune — all in one process.
//!   mundus-engine post-install --migrate-autostart --start-engine
//!       seed/migrate the Run key (StartupApproved opt-out honored) and/or
//!       start the installed Engine.
//!   mundus-engine kill-product-processes
//!       the single mop-up after a graceful `--shutdown`: kills only our
//!       own product image names via the Win32 API.
//!
//! Every subcommand prints one JSON outcome line and returns an exit code,
//! so the NSIS `nsExec::ExecToStack` result is inspectable.

mod autostart;
mod install;
mod legacy;
mod manifest;
mod post_install;
mod processes;
pub(crate) mod registry;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::process::ExitCode;

/// Called from `main` before `run` — when argv starts with one of the
/// installer subcommands the whole process is dedicated to it and never
/// starts the Engine runtime.
pub fn run_if_installer(args: &[String]) -> Option<ExitCode> {
    let code = match args.first().map(String::as_str) {
        Some("install") => install_cli(&args[1..]),
        Some("post-install") => post_install_cli(&args[1..]),
        Some("kill-product-processes") => processes::run_kill_product_processes(),
        _ => return None,
    };
    Some(code)
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).map(String::as_str))
}

fn print_outcome(result: Result<serde_json::Value, String>) -> ExitCode {
    match result {
        Ok(mut body) => {
            body["ok"] = serde_json::json!(true);
            println!("{body}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            println!("{}", serde_json::json!({ "ok": false, "error": error }));
            ExitCode::FAILURE
        }
    }
}

fn install_cli(args: &[String]) -> ExitCode {
    let (Some(manifest), Some(target_root)) = (
        flag_value(args, "--manifest"),
        flag_value(args, "--target-root"),
    ) else {
        return print_outcome(Err(
            "usage: mundus-engine install --manifest <path> --target-root <root>".into(),
        ));
    };
    print_outcome(install::install(&install::Options {
        manifest: PathBuf::from(manifest),
        target_root: PathBuf::from(target_root),
        legacy_registry_key: flag_value(args, "--legacy-registry-key")
            .map(str::to_owned)
            .unwrap_or_else(|| legacy::LEGACY_UNINSTALL_KEY.to_owned()),
        legacy_shortcut: flag_value(args, "--legacy-shortcut").map(PathBuf::from),
    }))
}

fn post_install_cli(args: &[String]) -> ExitCode {
    let engine_root = flag_value(args, "--engine-root")
        .map(PathBuf::from)
        .or_else(default_engine_root);
    let Some(engine_root) = engine_root else {
        return print_outcome(Err(
            "cannot resolve the Engine root (LOCALAPPDATA unset)".into()
        ));
    };
    print_outcome(post_install::post_install(&post_install::Options {
        engine_root,
        run_key: flag_value(args, "--run-key")
            .map(|s| registry::hkcu_subkey(s).to_owned())
            .unwrap_or_else(|| registry::RUN_SUBKEY.to_owned()),
        startup_approved_key: flag_value(args, "--startup-approved-key")
            .map(|s| registry::hkcu_subkey(s).to_owned())
            .unwrap_or_else(|| registry::APPROVED_SUBKEY.to_owned()),
        migrate_autostart: args.iter().any(|a| a == "--migrate-autostart"),
        start_engine: args.iter().any(|a| a == "--start-engine"),
    }))
}

/// `%LOCALAPPDATA%\Mundus\Engine` — the default install root the installer
/// passes explicitly; the flag exists so tests can point at a temp root.
fn default_engine_root() -> Option<PathBuf> {
    crate::data_dir::mundus_local_dir().map(|dir| dir.join(crate::brand::ENGINE_DIR_NAME))
}
