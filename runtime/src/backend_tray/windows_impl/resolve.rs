//! Impure resolution: reads env vars and probes the filesystem, using the
//! pure candidate lists from [`super::paths`]. Kept tiny and free of Win32
//! calls so it stays easy to reason about; see `paths.rs` for the tested
//! path-building logic this wraps.

use super::components::Component;
use super::paths::{component_executable_candidates, install_root_candidates};
use std::env;
use std::path::PathBuf;

const INSTALL_ROOT_ENV_VARS: [&str; 3] = ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"];

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

/// Resolves a launchable executable for a component. Manager is the bundled
/// `resources/components/manager` layout shipped next to the Engine;
/// Agenda/Memoria/Dictation are store-installed native apps resolved from
/// their install record under `Apps/<id>/<version>` — an app that isn't
/// installed simply isn't listed. Dev env overrides win in both paths.
pub fn resolve_component_executable(component: Component) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os(component.env_override()) {
        candidates.push(PathBuf::from(path));
    }
    if let Some(app_id) = component.app_id() {
        if let Ok(store) = engine::native_apps::default_store() {
            if let Some(path) = store.executable_path(app_id) {
                candidates.push(path);
            }
        }
        return candidates.into_iter().find(|path| path.is_file());
    }
    candidates.extend(component_executable_candidates(&install_roots(), component));
    candidates.into_iter().find(|path| path.is_file())
}
