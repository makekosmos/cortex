//! Takeover of a standalone "Kosmos Engine" Apps & Features registration
//! left by the old separate installer — port of `Invoke-EngineMigration`
//! from install-engine.ps1. Removes the registration and its Start Menu
//! shortcut, but never touches `%APPDATA%\Kosmos` (user data — the Engine
//! moves it on first start) or the `%LOCALAPPDATA%` Engine files.
//!
//! MIGRATION(KOS-267): remove after 2026-11-01.

use std::path::PathBuf;

/// Default HKCU subkey of the legacy standalone registration.
pub(crate) const LEGACY_UNINSTALL_KEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine"; // MIGRATION(KOS-267)

/// The user's Start Menu Programs folder, the same
/// `[Environment]::GetFolderPath('Programs')` the script used — resolved via
/// the known-folder API (`FOLDERID_Programs`), never guessed from %APPDATA%
/// (a missing env var would silently produce a CWD-relative path).
/// Shared with `native_apps::shortcuts` via `crate::win32::known_folder`.
#[cfg(windows)]
fn programs_dir() -> Result<PathBuf, String> {
    crate::win32::known_folder(&windows::Win32::UI::Shell::FOLDERID_Programs)
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::installer::registry::{self, RegValue};

    /// Values copied verbatim on rollback — same list the script snapshotted.
    const SNAPSHOT_VALUES: &[&str] = &[
        "DisplayName",
        "DisplayVersion",
        "Publisher",
        "InstallLocation",
        "UninstallString",
        "QuietUninstallString",
        "DisplayIcon",
        "NoModify",
        "NoRepair",
    ];

    /// Removes the legacy registration; Ok(false) when it is absent.
    /// `legacy_key`/`shortcut` are overridable only so tests can point at a
    /// scratch key/shortcut — the installer passes the real defaults.
    pub fn migrate_standalone_registration(
        legacy_key: &str,
        shortcut: Option<PathBuf>,
    ) -> Result<bool, String> {
        let key = registry::hkcu_subkey(legacy_key);
        if !registry::key_exists(key)? {
            return Ok(false);
        }
        let shortcut = match shortcut {
            Some(path) => path,
            None => programs_dir()?.join("Kosmos Engine.lnk"), // MIGRATION(KOS-267)
        };

        // Snapshot before changing anything; restore on failure — never
        // leave a half-migrated registration.
        let mut snapshot: Vec<(String, RegValue)> = Vec::new();
        for name in SNAPSHOT_VALUES {
            if let Some(value) = registry::read(key, name)? {
                snapshot.push(((*name).to_owned(), value));
            }
        }
        let install_location = registry::read_sz(key, "InstallLocation");

        let shortcut_backup =
            std::env::temp_dir().join(format!("mundus-engine-shortcut-{}.bak", std::process::id()));
        let had_shortcut = shortcut.is_file();
        if had_shortcut {
            std::fs::copy(&shortcut, &shortcut_backup)
                .map_err(|e| format!("backup {shortcut:?}: {e}"))?;
        }

        let result = (|| -> Result<(), String> {
            registry::delete_tree(key)?;
            if had_shortcut {
                std::fs::remove_file(&shortcut).map_err(|e| format!("remove {shortcut:?}: {e}"))?;
            }
            if let Some(location) = install_location {
                // Best-effort: an orphaned standalone Uninstall.exe is no
                // longer reachable from Apps & Features, but leaving it
                // around is dead weight.
                let _ = std::fs::remove_file(PathBuf::from(location).join("Uninstall.exe"));
            }
            Ok(())
        })();

        if let Err(error) = result {
            // Rollback: recreate the key with the snapshotted values and put
            // the shortcut back.
            let _ = registry::create_key(key);
            for (name, value) in &snapshot {
                let _ = registry::write(key, name, value);
            }
            if had_shortcut && shortcut_backup.is_file() {
                let _ = std::fs::copy(&shortcut_backup, &shortcut);
            }
            if had_shortcut {
                let _ = std::fs::remove_file(&shortcut_backup);
            }
            return Err(format!(
                "Kosmos Engine migration failed and was rolled back: {error}" // MIGRATION(KOS-267)
            ));
        }
        if had_shortcut {
            let _ = std::fs::remove_file(&shortcut_backup);
        }
        Ok(true)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn migrate_standalone_registration(
        _legacy_key: &str,
        _shortcut: Option<PathBuf>,
    ) -> Result<bool, String> {
        Ok(false)
    }
}

pub(crate) use imp::*;
