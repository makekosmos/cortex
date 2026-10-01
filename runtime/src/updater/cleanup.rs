//! Cleanup of `<data_dir>/updates/` (KOS-301).
//!
//! The service writes exactly two shapes here: the committed installer
//! (`downloads_dir.join(manifest file.url)`) and its in-progress sidecar
//! produced by [`part_path`]. Before this sweep both survived forever —
//! applied installers, superseded versions and `.part` files of crashed
//! downloads all accumulated.
//!
//! The keep set — built from `pending` state, not from file names — is the
//! source of truth: the pending installer and its `.part` always survive.
//! Everything else that is a regular file directly inside `updates/` is a
//! leftover from a previous run (applied, superseded or crashed); directories
//! and links are skipped, and files are never touched outside the dir.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// In-progress download sidecar for a pending installer —
/// `download_resumable` appends into it, `verify_and_commit` renames it
/// over the installer name. Shared by `service` and this sweep so both
/// agree on the convention.
pub(crate) fn part_path(installer_path: &Path) -> PathBuf {
    installer_path.with_extension("exe.part")
}

/// Keep set for the paths the updater itself knows are live: the pending
/// installer and its `.part` sidecar (the download resumes into it).
pub(crate) fn keep_set(installer_path: Option<&Path>) -> BTreeSet<PathBuf> {
    installer_path
        .into_iter()
        .flat_map(|path| [path.to_path_buf(), part_path(path)])
        .collect()
}

/// Статистика одного прохода — для логов и тестов.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// Удалённые leftover payload'ы.
    pub removed: usize,
    /// Удаления, завершившиеся ошибкой (залогированы, проход не прерывается).
    pub failed: usize,
}

/// Remove every regular file in `updates/` that is not in `keep`. Called at
/// service construction (keep is empty — a fresh service has no pending
/// update and no live download) and whenever `check` rewrites `pending`,
/// so a superseded or no-longer-available payload does not linger.
pub fn sweep_downloads(downloads_dir: &Path, keep: &BTreeSet<PathBuf>) -> SweepReport {
    let mut report = SweepReport::default();
    let Ok(entries) = std::fs::read_dir(downloads_dir) else {
        return report;
    };
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();
        if keep.contains(&path) {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => report.removed += 1,
            Err(error) => {
                report.failed += 1;
                eprintln!("[updater] sweep failed to remove {path:?}: {error}");
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, b"x").unwrap();
        path
    }

    #[test]
    fn stale_payloads_removed_pending_installer_and_part_survive() {
        let dir = tempfile::tempdir().unwrap();
        let updates = dir.path().join("updates");
        fs::create_dir(&updates).unwrap();
        let applied = write(&updates, "Mundus-Setup-1.0.0.exe");
        let crashed = write(&updates, "Mundus-Setup-1.1.0.exe.part");
        let pending = write(&updates, "Mundus-Setup-1.2.0.exe");
        let part = write(&updates, "Mundus-Setup-1.2.0.exe.part");

        let keep = keep_set(Some(&pending));
        let report = sweep_downloads(&updates, &keep);

        assert_eq!(report.removed, 2);
        assert!(!applied.exists() && !crashed.exists());
        assert!(pending.exists() && part.exists());
    }

    #[test]
    fn empty_keep_set_removes_all_files_but_not_dirs() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.exe");
        write(dir.path(), "b.exe.part");
        fs::create_dir(dir.path().join("subdir")).unwrap();

        let report = sweep_downloads(dir.path(), &BTreeSet::new());

        assert_eq!(report.removed, 2);
        assert!(dir.path().join("subdir").is_dir());
    }

    #[test]
    fn missing_dir_is_a_clean_noop() {
        let dir = tempfile::tempdir().unwrap();
        let report = sweep_downloads(&dir.path().join("updates"), &BTreeSet::new());
        assert_eq!(report, SweepReport::default());
    }

    #[test]
    fn nothing_outside_updates_dir_is_touched() {
        let root = tempfile::tempdir().unwrap();
        let updates = root.path().join("updates");
        fs::create_dir(&updates).unwrap();
        write(&updates, "old.exe");
        let sibling = write(root.path(), "engine.lock.json");

        sweep_downloads(&updates, &BTreeSet::new());

        assert!(sibling.exists());
        assert!(fs::read_dir(&updates).unwrap().next().is_none());
    }
}
