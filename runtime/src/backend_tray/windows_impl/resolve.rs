//! Impure resolution: reads env vars and probes the filesystem, using the
//! pure candidate lists from [`super::paths`]. Kept tiny and free of Win32
//! calls so it stays easy to reason about; see `paths.rs` for the tested
//! path-building logic this wraps.

use super::paths::{install_root_candidates, manager_executable_candidates};
use std::env;
use std::path::PathBuf;

const INSTALL_ROOT_ENV_VARS: [&str; 3] = ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"];
/// Dev override for the Manager executable, the same source `native_apps` uses.
const MANAGER_EXECUTABLE_ENV: &str = "MANAGER_EXECUTABLE";

fn install_roots() -> Vec<PathBuf> {
    let exe_parent = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.to_path_buf()));
    let env_roots: Vec<PathBuf> = INSTALL_ROOT_ENV_VARS
        .into_iter()
        .filter_map(|var| env::var_os(var).map(PathBuf::from))
        .collect();
    install_root_candidates(exe_parent.as_deref(), &env_roots)
}

fn env_override() -> Option<PathBuf> {
    let path = PathBuf::from(crate::brand::env_os(MANAGER_EXECUTABLE_ENV)?);
    path.is_file().then_some(path)
}

/// Resolves the Manager to launch from the tray: the dev env override first,
/// then the bundled `resources/components/manager` layout next to the Engine.
pub fn manager_executable() -> Option<PathBuf> {
    env_override().or_else(|| {
        manager_executable_candidates(&install_roots())
            .into_iter()
            .find(|path| path.is_file())
    })
}

/// Manager exe resolution for *authorization* (the `/v1/rpc` caller-identity
/// check). Unlike launch resolution, the `MUNDUS_MANAGER_EXECUTABLE` env
/// override exists only in debug builds: in a release Engine an env var must
/// never decide who may trigger the elevated install (round 4).
pub fn resolve_manager_for_auth() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(path) = env_override() {
        return Some(path);
    }
    manager_executable_candidates(&install_roots())
        .into_iter()
        .find(|path| path.is_file())
}
