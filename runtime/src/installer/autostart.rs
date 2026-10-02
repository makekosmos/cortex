//! Autostart seeding and migration — the in-process port of
//! `engine-post-install.ps1 -MigrateAutostart` and install-engine.ps1's
//! `Update-EngineAutostart` (KOS-306). The opt-out contract is unchanged: a
//! disabled `StartupApproved\Run` marker under the current OR any legacy
//! product name means the user turned autostart off and we keep it off.

use std::path::Path;

use serde::Serialize;

#[cfg(windows)]
use super::registry;
#[cfg(windows)]
use crate::brand;

/// Run value names that count as a persisted opt-out — the current name plus
/// every prior generation's. MIGRATION(KOS-267): remove the legacy names
/// after 2026-11-01.
const OPT_OUT_NAMES: &[&str] = &[
    "Mundus Engine",
    "Kosmos Engine",       // MIGRATION(KOS-267)
    "Kosmos",              // MIGRATION(KOS-267)
    "electron.app.Kosmos", // MIGRATION(KOS-267)
    "com.kazui.kosmos",    // MIGRATION(KOS-267)
    "com.kazui.kepler",    // MIGRATION(KOS-267)
    "Kepler",              // MIGRATION(KOS-267)
    "KeplerKosmos",        // MIGRATION(KOS-267)
    "KosmosKepler",        // MIGRATION(KOS-267)
];

#[derive(Debug, Serialize)]
#[serde(tag = "autostart")]
pub enum Seed {
    /// Run value + enabled marker written; `command` is the stored value.
    Written { command: String },
    /// A disabled StartupApproved marker exists — nothing written.
    OptOut { value_name: String },
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::manager_api::windows_autostart::approved_marker;

    /// Disabled marker check: state byte 3 (and 6 on some builds for items
    /// disabled at sign-in); 2 = enabled, absent = never toggled. A read
    /// failure is treated as absent — same as the script's
    /// `-ErrorAction SilentlyContinue`.
    fn opt_out_name(approved_key: &str) -> Option<String> {
        for name in OPT_OUT_NAMES {
            let marker = registry::read(approved_key, name).ok().flatten();
            if marker.is_some_and(|v| matches!(v.data.first(), Some(3 | 6))) {
                return Some((*name).to_owned());
            }
        }
        None
    }

    /// Seed (or migrate) the Run entry for `exe`. `run_key`/`approved_key`
    /// are the real subkeys in production; tests point them at scratch
    /// subkeys under `HKCU\Software\…`.
    pub fn seed_or_migrate(exe: &Path, run_key: &str, approved_key: &str) -> Result<Seed, String> {
        if let Some(name) = opt_out_name(approved_key) {
            return Ok(Seed::OptOut { value_name: name });
        }
        let command = format!("\"{}\" --start", exe.display());
        registry::write_sz(run_key, brand::AUTOSTART_RUN_VALUE, &command)?;
        // Mark the entry enabled so Task Manager shows the same state — the
        // write the OS itself makes on Enable.
        registry::write_binary(
            approved_key,
            brand::AUTOSTART_RUN_VALUE,
            &approved_marker(false),
        )?;
        Ok(Seed::Written { command })
    }

    /// Port of `Update-EngineAutostart`: after a (re)install, point an
    /// existing engine-autostart Run value at the just-installed exe. Only
    /// runs when `target_root` is the real default Engine root; errors are
    /// warnings — a stale autostart path never fails an install.
    pub fn update_for_install(version_root: &Path, target_root: &Path) {
        let result = (|| -> Result<(), String> {
            let Some(default_root) =
                crate::data_dir::mundus_local_dir().map(|dir| dir.join(brand::ENGINE_DIR_NAME))
            else {
                return Ok(());
            };
            let normalize = |p: &Path| {
                std::fs::canonicalize(p)
                    .unwrap_or_else(|_| p.to_path_buf())
                    .to_string_lossy()
                    .trim_end_matches('\\')
                    .to_owned()
            };
            if !normalize(target_root).eq_ignore_ascii_case(&normalize(&default_root)) {
                return Ok(());
            }
            let exe = version_root.join(format!("{}.exe", brand::ENGINE_BINARY_STEM));
            let command = format!("\"{}\" --start", exe.display());
            // MIGRATION(KOS-267): keep reading/rewriting the legacy Run value
            // names until cleanup; always write under the new name.
            for name in ["Mundus Engine", "Kosmos Engine"] {
                // MIGRATION(KOS-267)
                let Some(value) = registry::read_sz(registry::RUN_SUBKEY, name) else {
                    continue;
                };
                let lower = value.to_lowercase();
                let points_at_engine = lower.contains("--start")
                    && [
                        "kepler-backend.exe", // MIGRATION(KOS-267)
                        "mundus-engine.exe",
                        "kosmos runtime.exe", // MIGRATION(KOS-267)
                    ]
                    .iter()
                    .any(|name| lower.contains(name));
                if points_at_engine {
                    registry::write_sz(registry::RUN_SUBKEY, brand::AUTOSTART_RUN_VALUE, &command)?;
                }
                if name != brand::AUTOSTART_RUN_VALUE {
                    registry::delete_value(registry::RUN_SUBKEY, name);
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("[mundus-engine] engine autostart update skipped: {error}");
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn seed_or_migrate(
        _exe: &Path,
        _run_key: &str,
        _approved_key: &str,
    ) -> Result<Seed, String> {
        Err("autostart migration is Windows-only".into())
    }

    pub fn update_for_install(_version_root: &Path, _target_root: &Path) {}
}

pub(crate) use imp::*;
