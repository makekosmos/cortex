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

/// Test seam: [`FailNextMoveIn`] makes the next move-in on this thread fail
/// once, so the rollback path can be exercised without holding a real file lock.
#[cfg(test)]
fn move_in(temp: &Path, version_root: &Path) -> std::io::Result<()> {
    // Thread-local, not a process atomic: `cargo test` runs these tests in
    // one process, and a shared flag is consumed by whichever install runs
    // first. nextest (one process per test) hides that.
    if FAIL_NEXT_MOVE_IN.with(|flag| flag.replace(false)) {
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

/// Same seam as `move_in`: [`FailNextRestore`] makes the restore rename on
/// this thread fail once so the "no Engine left" error path is testable.
#[cfg(test)]
fn restore_aside(aside: &Path, version_root: &Path) -> std::io::Result<()> {
    if FAIL_NEXT_RESTORE.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "test seam",
        ));
    }
    std::fs::rename(aside, version_root)
}

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_MOVE_IN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_NEXT_RESTORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Arms the next `move_in` on this thread only. Drop disarms it so a
/// panicked test cannot leak the seam onto a reused test thread.
#[cfg(test)]
pub(crate) struct FailNextMoveIn;

#[cfg(test)]
impl FailNextMoveIn {
    pub(crate) fn arm() -> Self {
        FAIL_NEXT_MOVE_IN.with(|flag| flag.set(true));
        Self
    }
}

#[cfg(test)]
impl Drop for FailNextMoveIn {
    fn drop(&mut self) {
        FAIL_NEXT_MOVE_IN.with(|flag| flag.set(false));
    }
}

/// Arms the next restore rename on this thread only. See [`FailNextMoveIn`].
#[cfg(test)]
pub(crate) struct FailNextRestore;

#[cfg(test)]
impl FailNextRestore {
    pub(crate) fn arm() -> Self {
        FAIL_NEXT_RESTORE.with(|flag| flag.set(true));
        Self
    }
}

#[cfg(test)]
impl Drop for FailNextRestore {
    fn drop(&mut self) {
        FAIL_NEXT_RESTORE.with(|flag| flag.set(false));
    }
}

#[cfg(test)]
mod seam_tests {
    use super::{move_in, restore_aside, FailNextMoveIn, FailNextRestore};

    #[test]
    fn move_in_seam_stays_on_the_arming_thread() {
        let root = tempfile::tempdir().unwrap();
        let other_temp = root.path().join("other-temp");
        let other_dest = root.path().join("other-dest");
        std::fs::create_dir(&other_temp).unwrap();
        let own_temp = root.path().join("own-temp");
        let own_dest = root.path().join("own-dest");
        std::fs::create_dir(&own_temp).unwrap();

        let _guard = FailNextMoveIn::arm();
        let other_dest_check = other_dest.clone();
        let other = std::thread::spawn(move || move_in(&other_temp, &other_dest));
        assert!(
            other.join().unwrap().is_ok(),
            "a shared flag would fail the other thread's rename"
        );
        assert!(other_dest_check.is_dir());
        assert!(move_in(&own_temp, &own_dest).is_err());
        assert!(!own_dest.exists());
    }

    #[test]
    fn restore_seam_stays_on_the_arming_thread() {
        let root = tempfile::tempdir().unwrap();
        let other_aside = root.path().join("other-aside");
        let other_dest = root.path().join("other-live");
        std::fs::create_dir(&other_aside).unwrap();
        let own_aside = root.path().join("own-aside");
        let own_dest = root.path().join("own-live");
        std::fs::create_dir(&own_aside).unwrap();

        let _guard = FailNextRestore::arm();
        let other_dest_check = other_dest.clone();
        let other = std::thread::spawn(move || restore_aside(&other_aside, &other_dest));
        assert!(other.join().unwrap().is_ok());
        assert!(other_dest_check.is_dir());
        assert!(restore_aside(&own_aside, &own_dest).is_err());
        assert!(!own_dest.exists());
    }
}
