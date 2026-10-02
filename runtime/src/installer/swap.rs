//! Same-version replacement of `versions/<v>` — split from `install.rs`
//! to keep both files under the 300-line bar.

use std::path::Path;

use semver::Version;
use serde_json::Value;

/// Replace `version_root` with the staged `temp` dir as two renames, so a
/// failed move-in never leaves `current.json` pointing at a deleted Engine
/// (an AV lock or sharing violation between `remove_dir_all` and `rename`
/// used to destroy the live install). The old dir goes aside first and is
/// restored on failure; on success it is deleted best-effort.
pub(super) fn swap_in_version_dir(
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
            // Best-effort restore: a failed move-in is recoverable only if
            // the aside dir lands back — when even that fails, the error
            // must say where the previous Engine still is.
            return Err(match restore_aside(&aside, version_root) {
                Ok(()) => format!(
                    "rename {temp:?} -> {version_root:?}: {error} — previous Engine restored"
                ),
                Err(restore) => format!(
                    "rename {temp:?} -> {version_root:?}: {error}; rollback failed ({restore}) \
                     — previous Engine kept at {aside:?}"
                ),
            });
        }
        return Err(format!("rename {temp:?} -> {version_root:?}: {error}"));
    }
    if had_old {
        if let Err(error) = std::fs::remove_dir_all(&aside) {
            report["cleanup_warning"] = serde_json::json!(format!(
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

#[cfg(not(test))]
fn restore_aside(aside: &Path, version_root: &Path) -> std::io::Result<()> {
    std::fs::rename(aside, version_root)
}

/// Same seam as `move_in`: `FAIL_NEXT_RESTORE` makes the restore rename
/// fail once so the "no Engine left" error path is testable.
#[cfg(test)]
fn restore_aside(aside: &Path, version_root: &Path) -> std::io::Result<()> {
    if FAIL_NEXT_RESTORE.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "test seam",
        ));
    }
    std::fs::rename(aside, version_root)
}

#[cfg(test)]
pub(crate) static FAIL_NEXT_MOVE_IN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub(crate) static FAIL_NEXT_RESTORE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
