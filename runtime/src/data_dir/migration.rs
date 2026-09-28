//! First-start migration of user data from the 0.9.x-era directories to the
//! Mundus ones. Everything in this module is temporary code.
// MIGRATION(KOS-267): remove after 2026-11-01.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{legacy_config_dir, MIGRATION_MARKER};
use crate::singleton::{SingletonError, SingletonGuard};

#[derive(Debug, Default, Clone)]
pub struct MigrationReport {
    /// Roaming rename `%APPDATA%\Kosmos` → `%APPDATA%\Mundus` happened.
    pub roaming_migrated: bool,
    /// Local entries moved out of `%LOCALAPPDATA%\Kosmos`.
    pub local_entries_moved: usize,
    /// We are running on the legacy dir this session (rename failed or the
    /// legacy dir is still locked); retry happens on the next start.
    pub fell_back_to_legacy: bool,
}

/// Pure-ish migration step, split out for tests: given explicit legacy/new
/// dir pairs, performs the rename/move rules and reports what happened.
pub fn migrate_dirs(
    new_roaming: &Path,
    legacy_roaming: &Path,
    new_local: Option<PathBuf>,
    legacy_local: Option<PathBuf>,
) -> MigrationReport {
    let mut report = MigrationReport::default();

    let legacy_exists = legacy_roaming.is_dir();
    let new_exists = new_roaming.is_dir();

    if !legacy_exists {
        // Fresh install — nothing to move. Still let the local-dir migration
        // run: an old Engine payload dir never lives under roaming anyway.
    } else if new_exists {
        // A Mundus dir already exists — never merge. If it is non-empty this
        // is a normal post-migration start (or a manual split) and the legacy
        // dir is simply left alone; retry is a no-op.
    } else if legacy_dir_locked(legacy_roaming) {
        // An old Engine is running right now. Fall back so both engines keep
        // working; the next start retries the rename.
        tracing::warn!(
            target: "migration",
            dir = %legacy_roaming.display(),
            "legacy data dir is locked by a running Engine; using it this session"
        );
        report.fell_back_to_legacy = true;
    } else {
        match lock_file_retry_rename(legacy_roaming, new_roaming) {
            Ok(()) => {
                report.roaming_migrated = true;
                write_migration_marker(new_roaming);
                tracing::info!(
                    target: "migration",
                    from = %legacy_roaming.display(),
                    to = %new_roaming.display(),
                    "migrated roaming data dir"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: "migration",
                    %error,
                    from = %legacy_roaming.display(),
                    to = %new_roaming.display(),
                    "roaming dir rename failed; using legacy dir this session"
                );
                report.fell_back_to_legacy = true;
            }
        }
    }

    // Local dir: move non-Engine entries individually. The installer may have
    // already created `%LOCALAPPDATA%\Mundus\Engine`, so the target can exist
    // and be non-empty — we merge entry-by-entry and never overwrite.
    if let (Some(new_local), Some(legacy_local)) = (new_local, legacy_local) {
        report.local_entries_moved = migrate_local_dir(&new_local, &legacy_local);
    }

    report
}

/// Probe whether an old Engine currently holds the legacy dir: take the
/// legacy singleton lock. Acquiring it means nobody owns it.
fn legacy_dir_locked(legacy_roaming: &Path) -> bool {
    let lock = legacy_roaming.join(super::LEGACY_SINGLETON_LOCK_NAME);
    match SingletonGuard::acquire(&lock) {
        Ok(_guard) => false,
        Err(SingletonError::AlreadyRunning) => true,
        Err(_) => false, // unreadable/unusable lock — treat as free, rename tries anyway
    }
}

/// `MoveFileEx`-style rename with a couple of retries — transient AV/indexer
/// handles on Windows commonly make the first rename bounce.
fn lock_file_retry_rename(from: &Path, to: &Path) -> io::Result<()> {
    let mut last = fs::rename(from, to);
    for _ in 0..3 {
        if last.is_ok() {
            return last;
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
        last = fs::rename(from, to);
    }
    last
}

/// Move each entry of the legacy local dir into the Mundus local dir.
/// `Engine` is owned by the installer and never moved (the installer deletes
/// it after installing the new Engine). Existing targets are skipped, never
/// overwritten. Returns the number of moved entries.
fn migrate_local_dir(new_local: &Path, legacy_local: &Path) -> usize {
    let Ok(entries) = fs::read_dir(legacy_local) else {
        return 0;
    };
    let mut moved = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.eq_ignore_ascii_case("Engine") {
            continue;
        }
        let target = new_local.join(&name);
        if target.symlink_metadata().is_ok() {
            tracing::warn!(
                target: "migration",
                entry = %name.to_string_lossy(),
                "skipping legacy local entry: target exists"
            );
            continue;
        }
        if fs::create_dir_all(new_local).is_err() {
            continue;
        }
        match lock_file_retry_rename(&entry.path(), &target) {
            Ok(()) => moved += 1,
            Err(error) => tracing::warn!(
                target: "migration",
                entry = %name.to_string_lossy(),
                %error,
                "legacy local entry move failed; left in place"
            ),
        }
    }
    if moved > 0 {
        tracing::info!(
            target: "migration",
            moved,
            from = %legacy_local.display(),
            to = %new_local.display(),
            "migrated local data dir entries"
        );
    }
    moved
}

fn write_migration_marker(new_roaming: &Path) {
    let marker = new_roaming.join(MIGRATION_MARKER);
    let body = serde_json::json!({
        "from": "Kosmos", // MIGRATION(KOS-267)
        "to": crate::brand::PRODUCT_NAME,
        "at": chrono::Utc::now().to_rfc3339(),
    });
    if let Err(error) = fs::write(
        &marker,
        serde_json::to_vec_pretty(&body).unwrap_or_default(),
    ) {
        tracing::warn!(target: "migration", %error, "failed to write migration marker");
    }
}

/// Engine lock-file shim for pinned component builds that still read only
/// `%APPDATA%\Kosmos\engine.lock.json`. Called after the real lock write.
/// Creates the legacy dir holding just the lock file when the migration has
/// already renamed it away — the dir is a pointer, not user data.
// MIGRATION(KOS-267): remove after 2026-11-01.
pub fn write_legacy_lock_shim(lock: &crate::lock_file::EngineLockFile) {
    // A MUNDUS_DATA_DIR / KOSMOS_DATA_DIR override means a dev/test engine —
    // never let it publish to the real legacy location.
    if super::has_env_override() {
        return;
    }
    let Ok(legacy) = legacy_config_dir() else {
        return;
    };
    if fs::create_dir_all(&legacy).is_err() {
        return;
    }
    let legacy_lock = legacy.join(crate::lock_file::ENGINE_LOCK_FILE_NAME);
    if let Err(error) = crate::lock_file::write_engine_atomic(&legacy_lock, lock) {
        tracing::warn!(target: "migration", %error, "legacy engine.lock.json shim write failed");
    }
}

/// Remove the legacy lock shim on shutdown so a stale file does not point a
/// pinned component at a dead Engine.
// MIGRATION(KOS-267): remove after 2026-11-01.
pub fn remove_legacy_lock_shim() {
    let Ok(legacy) = legacy_config_dir() else {
        return;
    };
    let path = legacy.join(crate::lock_file::ENGINE_LOCK_FILE_NAME);
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}
