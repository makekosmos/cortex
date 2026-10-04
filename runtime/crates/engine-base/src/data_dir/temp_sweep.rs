//! Sweep of interrupted atomic-write leftovers (KOS-301).
//!
//! Several modules write into the Engine data dir via the same temp+rename
//! shape; a crash between `File::create` and `rename` leaves the temp file
//! forever (e.g. `.protocol-usage.json.tmp.1234`). Every writer sweeps only
//! its own directory, only names matching that writer's own temp convention,
//! and only files older than [`LEFTOVER_GRACE`] — a live atomic write holds
//! its temp for seconds, so the grace window (same reasoning as KOS-300's
//! `RESTORE_TEMP_GRACE`) is what protects an in-flight write without any
//! cross-process flag.
//!
//! The sweep never descends into directories and never touches entries it
//! cannot classify — no globs over user paths.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// Temp entries younger than this may belong to a write in progress right
/// now (or a suspended process about to finish one) — they are kept.
pub const LEFTOVER_GRACE: Duration = Duration::from_secs(60 * 60);

/// Статистика одного прохода — для логов и тестов.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// Удалённые leftover'ы.
    pub removed: usize,
    /// Совпавшие по имени, но моложе grace — живой writer.
    pub kept_young: usize,
    /// Удаления, завершившиеся ошибкой (залогированы, проход не прерывается).
    pub failed: usize,
}

/// Remove entries directly inside `dir` whose name matches `is_leftover`
/// (called with the file name and whether the entry is a directory) and
/// whose mtime is older than `min_age`. Non-recursive; symlinks are never
/// followed or removed. Per-entry failures are logged and do not abort the
/// pass — one locked file must not block the rest of the cleanup.
pub fn sweep(
    dir: &Path,
    min_age: Duration,
    is_leftover: impl Fn(&str, bool) -> bool,
) -> SweepReport {
    let mut report = SweepReport::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return report;
    };
    let cutoff = SystemTime::now() - min_age;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let is_dir = file_type.is_dir();
        if !is_dir && !file_type.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_leftover(&name, is_dir) {
            continue;
        }
        let path = entry.path();
        let old_enough = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .is_ok_and(|mtime| mtime < cutoff);
        if !old_enough {
            report.kept_young += 1;
            continue;
        }
        let result = if is_dir {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match result {
            Ok(()) => report.removed += 1,
            Err(error) => {
                report.failed += 1;
                eprintln!("[data-dir] sweep failed to remove {path:?}: {error}");
            }
        }
    }
    report
}

/// Sweep the `<data_dir>` root for temp files of every atomic writer that
/// targets it:
/// - `.<name>.tmp.<pid>` / `.<name>.tmp.<pid>.<nonce>` —
///   `lock_file::write_owner_only_json` (engine.lock.json,
///   protocol-usage.json, …), `pomodoro_host`, `dictation::config`;
/// - `<name>.json.tmp` and `<name>.json.tmp.<pid>` — `integrations::config`,
///   `grant_authority`, `engine_settings`.
///
/// Both shapes are anchored on the `.tmp` infix those writers bake into the
/// name, so a real JSON/user file at the root is never matched.
pub fn sweep_engine_root(data_dir: &Path) -> SweepReport {
    sweep(data_dir, LEFTOVER_GRACE, |name, is_dir| {
        !is_dir && is_root_temp_name(name)
    })
}

fn is_root_temp_name(name: &str) -> bool {
    (name.starts_with('.') && name.contains(".tmp.")) || name.contains(".json.tmp")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, b"x").unwrap();
        path
    }

    fn age(path: &Path, secs: u64) {
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(secs))
            .unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn removes_old_matching_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let stale = write(dir.path(), ".engine.lock.json.tmp.42");
        age(&stale, 2 * 60 * 60);
        write(dir.path(), ".engine.lock.json.tmp.99"); // young — live write
        write(dir.path(), "engine.lock.json");
        write(dir.path(), "ark.db");

        let report = sweep(dir.path(), LEFTOVER_GRACE, |name, is_dir| {
            !is_dir && name.starts_with('.') && name.contains(".tmp.")
        });

        assert_eq!(report.removed, 1);
        assert_eq!(report.kept_young, 1);
        assert_eq!(
            names(dir.path()),
            vec![".engine.lock.json.tmp.99", "ark.db", "engine.lock.json"]
        );
    }

    #[test]
    fn engine_root_covers_every_root_writer_shape() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            ".engine.lock.json.tmp.42",
            ".protocol-usage.json.tmp.42",
            ".pomodoro-state.json.tmp.42",
            ".dictation-config.json.tmp.42.nonce",
            "integrations.json.tmp",
            "grant-authority.json.tmp",
            "engine-manager-settings.json.tmp.42",
        ] {
            age(&write(dir.path(), name), 2 * 60 * 60);
        }
        write(dir.path(), "ark.db");
        write(dir.path(), "integrations.json");
        write(dir.path(), "notes.json.tmpx"); // не наш shape

        let report = sweep_engine_root(dir.path());

        assert_eq!(report.removed, 7);
        assert_eq!(names(dir.path()).len(), 3);
    }

    #[test]
    fn matching_dirs_are_removed_when_old_enough() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".staging-dead-0")).unwrap();
        fs::create_dir(dir.path().join("unpacked")).unwrap();
        // Duration::ZERO stands in for an aged mtime — set_modified does not
        // portably apply to directories.
        let report = sweep(dir.path(), Duration::ZERO, |name, is_dir| {
            is_dir && name.starts_with(".staging-")
        });
        assert_eq!(report.removed, 1);
        assert_eq!(names(dir.path()), vec!["unpacked"]);
    }

    #[test]
    fn missing_dir_is_a_clean_noop() {
        let dir = tempfile::tempdir().unwrap();
        let report = sweep_engine_root(&dir.path().join("nonexistent"));
        assert_eq!(report, SweepReport::default());
    }

    #[test]
    fn never_touches_files_outside_the_dir() {
        let root = tempfile::tempdir().unwrap();
        let sub = root.path().join("packages");
        fs::create_dir(&sub).unwrap();
        age(&write(&sub, ".state.json.tmp.42"), 2 * 60 * 60);
        age(&write(root.path(), ".engine.lock.json.tmp.42"), 2 * 60 * 60);

        sweep(&sub, LEFTOVER_GRACE, |name, _| name.contains(".tmp."));

        assert!(!sub.join(".state.json.tmp.42").exists());
        assert!(root.path().join(".engine.lock.json.tmp.42").exists());
    }
}
