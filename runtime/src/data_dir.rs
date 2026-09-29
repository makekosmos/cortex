//! Mundus data-directory resolution and the 0.9.x → 0.10.0 on-disk
//! migration.
//!
//! Layout:
//!   roaming  `%APPDATA%\Mundus`      (legacy: `%APPDATA%\` + old dir name)
//!   local    `%LOCALAPPDATA%\Mundus` (legacy: `%LOCALAPPDATA%\` + old name;
//!            Engine payload lives under `<local>\Engine`, installer-owned)
//!
//! Migration rules (KOS-266):
//!   * If the Mundus roaming dir does not exist and the legacy one does, the
//!     Engine atomically renames it (`MoveFileEx`/`std::fs::rename`, same
//!     volume). Never copy-then-delete, never merge into a non-empty target.
//!   * The rename happens only while no old Engine holds the legacy dir —
//!     we probe the legacy singleton lock first.
//!   * If the rename fails we run on the legacy dir for this session, log it,
//!     and retry on the next start. We never start on an empty Mundus dir
//!     while legacy data exists.
//!   * For the local dir we move the non-`Engine` contents of the legacy dir
//!     into the (possibly already existing — the installer creates
//!     `Mundus\Engine` first) Mundus dir, entry by entry.
//!   * Idempotent; a marker file records the migration in the new dir.
//!
//! Everything below touching the legacy paths is
// MIGRATION(KOS-267): remove after 2026-11-01.

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::brand;
use crate::lock_file::LockFileError;

mod migration;
#[cfg(test)]
mod tests;

pub use migration::{remove_legacy_lock_shim, write_legacy_lock_shim, MigrationReport};

/// Marker written into the new roaming dir after a successful migration.
const MIGRATION_MARKER: &str = "mundus-migration.json";

// MIGRATION(KOS-267): remove after 2026-11-01.
const LEGACY_CONFIG_DIR_NAME: &str = "Kosmos"; // MIGRATION(KOS-267)
                                               // MIGRATION(KOS-267): remove after 2026-11-01.
const LEGACY_LOCAL_DIR_NAME: &str = "Kosmos"; // MIGRATION(KOS-267)
                                              // MIGRATION(KOS-267): remove after 2026-11-01.
const LEGACY_SINGLETON_LOCK_NAME: &str = "kepler-singleton.lock.db"; // MIGRATION(KOS-267)

/// Roaming config root: `%APPDATA%` / `$XDG_CONFIG_HOME` or `~/.config`.
fn config_root() -> Result<PathBuf, LockFileError> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let trimmed = appdata.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    #[cfg(unix)]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            let trimmed = xdg.trim();
            if !trimmed.is_empty() {
                return Ok(PathBuf::from(trimmed));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let trimmed = home.trim();
            if !trimmed.is_empty() {
                return Ok(PathBuf::from(trimmed).join(".config"));
            }
        }
    }
    Err(LockFileError::Io(io_error(
        "APPDATA (or XDG_CONFIG_HOME) is not set",
    )))
}

/// Local data root: `%LOCALAPPDATA%` / `$XDG_DATA_HOME` or
/// `~/.local/share`. Optional because a missing local dir is not fatal for
/// the roaming migration.
fn local_root() -> Option<PathBuf> {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let trimmed = local.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    #[cfg(unix)]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            let trimmed = xdg.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let trimmed = home.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed).join(".local").join("share"));
            }
        }
    }
    None
}

pub fn mundus_config_dir() -> Result<PathBuf, LockFileError> {
    Ok(config_root()?.join(brand::CONFIG_DIR_NAME))
}

/// Legacy roaming dir (`%APPDATA%\Kosmos`). Still inspected for migration.
// MIGRATION(KOS-267): remove after 2026-11-01.
pub fn legacy_config_dir() -> Result<PathBuf, LockFileError> {
    Ok(config_root()?.join(LEGACY_CONFIG_DIR_NAME))
}

pub fn mundus_local_dir() -> Option<PathBuf> {
    local_root().map(|root| root.join(brand::LOCAL_DIR_NAME))
}

/// Product-local base for installer-owned payloads (native apps live under
/// `<dir>/Apps`): env override first, then the Mundus local dir — or the
/// legacy local dir while roaming stayed on legacy this session.
pub fn mundus_local_data_dir() -> Option<PathBuf> {
    if let Some(dir) = env_data_dir_override() {
        return Some(dir);
    }
    if last_report().is_some_and(|report| report.fell_back_to_legacy) {
        legacy_local_dir()
    } else {
        mundus_local_dir()
    }
}

/// Legacy local dir (`%LOCALAPPDATA%\Kosmos`). Still inspected for migration.
// MIGRATION(KOS-267): remove after 2026-11-01.
pub fn legacy_local_dir() -> Option<PathBuf> {
    local_root().map(|root| root.join(LEGACY_LOCAL_DIR_NAME))
}

fn env_data_dir_override() -> Option<PathBuf> {
    brand::env_os("DATA_DIR").map(PathBuf::from)
}

static RESOLVED: OnceLock<PathBuf> = OnceLock::new();
static REPORT: std::sync::Mutex<Option<MigrationReport>> = std::sync::Mutex::new(None);

/// Resolve the Engine data directory: env override, else the Mundus roaming
/// dir, migrating from the legacy data dir on first start. If the rename
/// fails (or the legacy dir is still held by a running old Engine), falls
/// back to the legacy dir for this session.
pub fn mundus_data_dir() -> Result<PathBuf, LockFileError> {
    if let Some(dir) = env_data_dir_override() {
        return Ok(dir);
    }
    if let Some(dir) = RESOLVED.get() {
        return Ok(dir.clone());
    }
    let dir = resolve_or_migrate()?;
    let _ = RESOLVED.set(dir.clone());
    Ok(dir)
}

/// The migration report of the first [`mundus_data_dir`] resolution, for
/// logging after tracing is initialised (migration runs before it).
pub fn last_report() -> Option<MigrationReport> {
    REPORT.lock().ok()?.clone()
}

/// Singleton lock file name for a given data dir. On the legacy dir we must
/// use the legacy name (`kepler-singleton.lock.db`) so a running 0.9.x Engine
/// and this one still exclude each other.
// MIGRATION(KOS-267): remove the legacy branch after 2026-11-01.
pub fn singleton_lock_name(data_dir: &std::path::Path) -> &'static str {
    match legacy_config_dir() {
        Ok(legacy) if legacy == data_dir => LEGACY_SINGLETON_LOCK_NAME,
        _ => brand::SINGLETON_LOCK_NAME,
    }
}

/// True when the data dir came from the user/test env override.
pub fn has_env_override() -> bool {
    env_data_dir_override().is_some()
}

fn resolve_or_migrate() -> Result<PathBuf, LockFileError> {
    let new_dir = mundus_config_dir()?;
    let legacy_dir = legacy_config_dir()?;

    let report = migration::migrate_dirs(
        &new_dir,
        &legacy_dir,
        mundus_local_dir(),
        legacy_local_dir(),
    );

    let data_dir = if report.fell_back_to_legacy {
        legacy_dir
    } else {
        new_dir
    };
    if let Ok(mut slot) = REPORT.lock() {
        *slot = Some(report);
    }
    Ok(data_dir)
}

fn io_error(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, message.to_string())
}
