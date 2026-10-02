//! `mundus-engine post-install` — port of engine-post-install.ps1
//! (KOS-306). Resolves the installed Engine from `<engine_root>\current.json`
//! failing closed if the pointer or binary is missing, then:
//!
//!   --migrate-autostart   write HKCU Run "Mundus Engine" = "<exe>" --start
//!                         unless a persisted StartupApproved opt-out exists
//!   --start-engine        launch the installed Engine without waiting
//!
//! `--engine-root`, `--run-key` and `--startup-approved-key` are test seams;
//! the installer never passes them and the real defaults apply.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{autostart, manifest};

pub struct Options {
    pub engine_root: PathBuf,
    pub run_key: String,
    pub startup_approved_key: String,
    pub migrate_autostart: bool,
    pub start_engine: bool,
}

/// Resolve `<root>\versions\<current>\mundus-engine.exe` — fails closed on a
/// missing/invalid pointer or a missing binary.
pub fn resolve_engine_exe(engine_root: &Path) -> Result<PathBuf, String> {
    let pointer_path = engine_root.join("current.json");
    let pointer: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&pointer_path).map_err(|e| format!("read {pointer_path:?}: {e}"))?,
    )
    .map_err(|_| "invalid current.json".to_owned())?;
    if pointer.get("schema_version").and_then(|v| v.as_u64()) != Some(1) {
        return Err("invalid current.json".into());
    }
    let version = pointer
        .get("version")
        .and_then(|v| v.as_str())
        .and_then(manifest::strict_version)
        .ok_or("invalid current.json")?;
    let exe = engine_root
        .join("versions")
        .join(version.to_string())
        .join(format!("{}.exe", crate::brand::ENGINE_BINARY_STEM));
    if !exe.is_file() {
        return Err(format!("engine binary missing: {exe:?}"));
    }
    Ok(exe)
}

pub fn post_install(options: &Options) -> Result<Value, String> {
    if !options.migrate_autostart && !options.start_engine {
        return Err(
            "post-install: nothing to do (pass --migrate-autostart and/or --start-engine)".into(),
        );
    }
    let exe = resolve_engine_exe(&options.engine_root)?;
    let mut report = json!({ "engine_exe": exe.to_string_lossy() });

    if options.migrate_autostart {
        match autostart::seed_or_migrate(&exe, &options.run_key, &options.startup_approved_key)? {
            autostart::Seed::Written { command } => {
                report["autostart"] = json!(format!("{command} -> 'Mundus Engine'"));
            }
            autostart::Seed::OptOut { value_name } => {
                report["autostart"] = json!(format!("skipped opt-out={value_name}"));
            }
        }
    }
    if options.start_engine {
        super::install::start_engine(&exe)?;
        report["started"] = json!(true);
    }
    Ok(report)
}
