//! `mundus-engine install` — in-process port of install-engine.ps1
//! (KOS-306). The NSIS installer stages the Engine payload unpacked under
//! `resources\engine\` and runs the staged exe directly:
//!
//!   mundus-engine.exe install --manifest <engine-manifest.json> --target-root <root>
//!
//! The payload dir is the manifest's directory. Semantics are unchanged:
//! install into `<root>/versions/<v>`, verify every file against the
//! manifest hashes, never downgrade a newer verified install, replace a
//! same-version rebuild, take over the standalone legacy registration, then
//! restart / reseed autostart / prune — and print one JSON outcome line.

use std::path::{Path, PathBuf};

use semver::Version;
use serde_json::{json, Value};

use super::{autostart, legacy, manifest, processes};

pub struct Options {
    pub manifest: PathBuf,
    pub target_root: PathBuf,
    /// MIGRATION(KOS-267): test seams only — the installer always passes the
    /// real defaults.
    pub legacy_registry_key: String,
    pub legacy_shortcut: Option<PathBuf>,
}

/// Installed+verified version at `root`, or None — `Test-InstalledEngine`.
fn installed_version(root: &Path) -> Option<Version> {
    let pointer_path = root.join("current.json");
    let pointer: serde_json::Value =
        serde_json::from_slice(&std::fs::read(pointer_path).ok()?).ok()?;
    if pointer.get("schema_version").and_then(|v| v.as_u64()) != Some(1) {
        return None;
    }
    let version = pointer
        .get("version")
        .and_then(|v| v.as_str())
        .and_then(manifest::strict_version)?;
    let version_root = root.join("versions").join(version.to_string());
    let installed = manifest::load_installed(&version_root)?;
    if installed.version != version || !manifest::verify_installed(&version_root, &installed) {
        return None;
    }
    Some(version)
}

/// Stop a running Engine in `dir` before its files are replaced
/// (`Stop-EngineForReplacement`). Returns true when one was running.
fn stop_engines_in(dir: &Path) -> Result<bool, String> {
    let mut was_running = false;
    for name in processes::ENGINE_BINARY_NAMES {
        let exe = dir.join(name);
        if exe.is_file() {
            was_running |= processes::stop_for_replacement(&exe)?;
        }
    }
    Ok(was_running)
}

/// Atomic write of `{"schema_version":1,"version":v}` to `current.json` —
/// temp sibling + rename, the same replace semantics as `Move-Item -Force`.
fn write_current_pointer(current: &Path, version: &Version) -> Result<(), String> {
    let temp = current.with_extension(format!("json.{}.tmp", std::process::id()));
    let body = json!({ "schema_version": 1, "version": version.to_string() });
    std::fs::write(&temp, body.to_string()).map_err(|e| format!("write {temp:?}: {e}"))?;
    std::fs::rename(&temp, current).map_err(|e| format!("rename {temp:?} -> {current:?}: {e}"))
}

/// Launch the installed exe with `--start`, detached — the Engine runs
/// without a console window because the shipped exe is a GUI-subsystem
/// binary.
pub(super) fn start_engine(exe: &Path) -> Result<(), String> {
    std::process::Command::new(exe)
        .arg("--start")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("spawn {exe:?} --start: {e}"))
}

pub fn install(options: &Options) -> Result<Value, String> {
    let expected = manifest::load(&options.manifest)?;
    let payload_dir = options
        .manifest
        .parent()
        .ok_or_else(|| format!("manifest has no directory: {:?}", options.manifest))?;
    // Verify the staged payload before touching anything — the unpacked
    // equivalent of the old archive_sha256 check.
    for file in &expected.files {
        manifest::verify_file(&payload_dir.join(&file.name), file)?;
    }

    let installed = installed_version(&options.target_root);
    let mut report = json!({ "version": expected.version.to_string() });
    if let Some(installed) = &installed {
        let up_to_date = *installed > expected.version
            || (*installed == expected.version
                && manifest::same_build(
                    &options
                        .target_root
                        .join("versions")
                        .join(installed.to_string()),
                    &expected,
                ));
        if up_to_date {
            // Monotonic: never replace a newer verified Engine nor the same
            // build; still refresh a stale autostart path.
            autostart::update_for_install(
                &options
                    .target_root
                    .join("versions")
                    .join(installed.to_string()),
                &options.target_root,
            );
            report["action"] = json!("kept");
            report["installed_version"] = json!(installed.to_string());
            // MIGRATION(KOS-267): the standalone-registration takeover runs
            // on every install, kept or not — same as the script's tail.
            if legacy::migrate_standalone_registration(
                &options.legacy_registry_key,
                options.legacy_shortcut.clone(),
            )? {
                report["legacy_registration_removed"] = json!(true);
            }
            return Ok(report);
        }
    }

    let versions_root = options.target_root.join("versions");
    let version_root = versions_root.join(expected.version.to_string());
    let temp = versions_root.join(format!("{}.{}.tmp", expected.version, std::process::id()));
    // `version` is strict semver — it can never escape `versions/` — but a
    // malformed target_root still must not let the staging dir land outside
    // it; mirror the script's guard cheaply.
    if !temp.starts_with(&versions_root) || !version_root.starts_with(&versions_root) {
        return Err("invalid Engine installation path".into());
    }

    // Graceful shutdown before file replacement — current install first,
    // then the version dir being replaced (same-version rebuild).
    let mut engine_was_running = false;
    if let Some(installed) = &installed {
        engine_was_running |= stop_engines_in(&versions_root.join(installed.to_string()))?;
    }
    if version_root.is_dir() {
        engine_was_running |= stop_engines_in(&version_root)?;
    }
    // Pre-in-process Engines spawned an ark-core-rpc sidecar that survives a
    // backend kill and still holds the DB open.
    // MIGRATION(KOS-267): remove after 2026-11-01.
    let rpc_kill = processes::kill_by_names(&["ark-core-rpc.exe"])?;
    if !rpc_kill.failed.is_empty() {
        return Err(format!(
            "ark-core-rpc.exe still running: {:?}",
            rpc_kill.failed
        ));
    }

    if temp.exists() {
        std::fs::remove_dir_all(&temp).map_err(|e| format!("remove {temp:?}: {e}"))?;
    }
    std::fs::create_dir_all(&temp).map_err(|e| format!("create {temp:?}: {e}"))?;
    // Copy the manifest-listed payload plus the manifest itself — the
    // installed version dir keeps engine-manifest.json for later
    // verification, exactly like the zip extraction did.
    let stage = |name: &str| -> Result<(), String> {
        std::fs::copy(payload_dir.join(name), temp.join(name))
            .map(|_| ())
            .map_err(|e| format!("copy payload {name}: {e}"))
    };
    for file in &expected.files {
        stage(&file.name)?;
    }
    stage("engine-manifest.json")?;
    for file in &expected.files {
        manifest::verify_file(&temp.join(&file.name), file)?;
    }

    std::fs::create_dir_all(&versions_root)
        .map_err(|e| format!("create {versions_root:?}: {e}"))?;
    swap_in_version_dir(
        &temp,
        &version_root,
        &versions_root,
        &expected.version,
        &mut report,
    )?;
    for file in &expected.files {
        manifest::verify_file(&version_root.join(&file.name), file)
            .map_err(|_| format!("installed Engine file mismatch: {}", file.name))?;
    }

    // Monotonic guard on the pointer itself: a concurrent install of a
    // newer version must not be clobbered by this one finishing second.
    let pointer_version = installed_version(&options.target_root);
    if pointer_version.is_none_or(|pointer| expected.version > pointer) {
        write_current_pointer(&options.target_root.join("current.json"), &expected.version)?;
    }
    report["action"] = json!("installed");

    if engine_was_running {
        report["engine_was_running"] = json!(true);
        if let Err(error) =
            start_engine(&version_root.join(format!("{}.exe", crate::brand::ENGINE_BINARY_STEM)))
        {
            // Best-effort like the script's catch: the end-of-install start
            // retries anyway.
            report["restart_warning"] = json!(format!(
                "Engine was replaced but could not be restarted: {error}"
            ));
        }
    }
    autostart::update_for_install(&version_root, &options.target_root);

    // KOS-261: prune superseded versions/<v> dirs — keep current + one
    // previous. Runs in-process now (no `prune-versions` child process, no
    // temp log file); any failure is a warning, never a failed install.
    match crate::engine_versions::prune_at(&options.target_root) {
        Ok(Some(pruned)) => report["pruned"] = json!(pruned),
        Ok(None) => {}
        Err(error) => {
            report["prune_warning"] = json!(format!("engine versions prune failed: {error}"))
        }
    }

    // MIGRATION(KOS-267): remove after 2026-11-01 — the standalone
    // "Kosmos Engine" Apps & Features takeover runs last, after the new
    // Engine is installed and verified.
    let migrated = legacy::migrate_standalone_registration(
        &options.legacy_registry_key,
        options.legacy_shortcut.clone(),
    )?;
    if migrated {
        report["legacy_registration_removed"] = json!(true);
    }

    Ok(report)
}

/// Replace `version_root` with the staged `temp` dir as two renames, so a
/// failed move-in never leaves `current.json` pointing at a deleted Engine
/// (an AV lock or sharing violation between `remove_dir_all` and `rename`
/// used to destroy the live install). The old dir goes aside first and is
/// restored on failure; on success it is deleted best-effort.
fn swap_in_version_dir(
    temp: &Path,
    version_root: &Path,
    versions_root: &Path,
    version: &Version,
    report: &mut Value,
) -> Result<(), String> {
    let aside = versions_root.join(format!("{version}.{}.old", std::process::id()));
    if aside.exists() {
        std::fs::remove_dir_all(&aside).map_err(|e| format!("remove stale {aside:?}: {e}"))?;
    }
    let had_old = version_root.exists();
    if had_old {
        std::fs::rename(version_root, &aside)
            .map_err(|e| format!("rename {version_root:?} -> {aside:?}: {e}"))?;
    }
    if let Err(error) = move_in(temp, version_root) {
        if had_old {
            // Best-effort restore: the rename-back either succeeds or the
            // aside dir stays next to the missing version_root for repair.
            let _ = std::fs::rename(&aside, version_root);
        }
        return Err(format!("rename {temp:?} -> {version_root:?}: {error}"));
    }
    if had_old {
        if let Err(error) = std::fs::remove_dir_all(&aside) {
            report["cleanup_warning"] = json!(format!(
                "replaced Engine dir {aside:?} could not be removed: {error}"
            ));
        }
    }
    Ok(())
}

#[cfg(not(test))]
fn move_in(temp: &Path, version_root: &Path) -> std::io::Result<()> {
    std::fs::rename(temp, version_root)
}

/// Test seam: `FAIL_NEXT_MOVE_IN` makes the next move-in fail once, so the
/// rollback path can be exercised without holding a real file lock.
#[cfg(test)]
fn move_in(temp: &Path, version_root: &Path) -> std::io::Result<()> {
    if FAIL_NEXT_MOVE_IN.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "test seam",
        ));
    }
    std::fs::rename(temp, version_root)
}

#[cfg(test)]
pub(crate) static FAIL_NEXT_MOVE_IN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
