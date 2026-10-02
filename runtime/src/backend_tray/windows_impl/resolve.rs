//! Impure resolution: reads env vars and probes the filesystem, using the
//! pure candidate lists from [`super::paths`]. Kept tiny and free of Win32
//! calls so it stays easy to reason about; see `paths.rs` for the tested
//! path-building logic this wraps.

use super::components::Component;
use super::paths::{install_root_candidates, manager_executable_candidates};
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
/// `resources/components/manager` layout shipped next to the Engine; apps
/// are store-installed and resolve through their `Apps/<id>` install record
/// — read-only, the Apps dir is not created here. Dev env overrides win in
/// both paths.
pub fn resolve_component_executable(component: Component) -> Option<PathBuf> {
    if let Some(path) = crate::brand::env_os(component.env_override()) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(desc) = component.app_descriptor() {
        return crate::native_apps::native_apps_root()
            .ok()
            .and_then(|root| {
                crate::native_apps::NativeAppStore::at(root).executable_path(desc.id)
            });
    }
    manager_executable_candidates(&install_roots())
        .into_iter()
        .find(|path| path.is_file())
}

/// Manager exe resolution for *authorization* (the `/v1/rpc` caller-identity
/// check). Unlike launch resolution, the `MUNDUS_MANAGER_EXECUTABLE` env
/// override exists only in debug builds: in a release Engine an env var must
/// never decide who may trigger the elevated install (KOS-269 round 4). The
/// tray's launch path keeps honouring it — that is pre-existing behaviour
/// and not an auth source.
pub fn resolve_manager_for_auth() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(path) = crate::brand::env_os(Component::Manager.env_override()) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    manager_executable_candidates(&install_roots())
        .into_iter()
        .find(|path| path.is_file())
}
