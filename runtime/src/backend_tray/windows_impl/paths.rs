//! Pure path-building logic for locating the Cortex shell install and the
//! GPUI components packaged alongside it. Kept free of `std::env` and
//! filesystem access so the candidate lists can be tested without touching
//! a real machine; [`super::resolve`] does the impure env/filesystem work.

use super::components::Component;
use std::path::{Path, PathBuf};

/// Directories that may hold the packaged Cortex shell (`Kosmos.exe`) and,
/// alongside it, `resources/components/<name>/...`. Mirrors the
/// electron-builder NSIS layout: a per-user install under
/// `%LOCALAPPDATA%\Programs\Kosmos`, or a plain `<root>\Kosmos` layout under
/// any of `LOCALAPPDATA` / `ProgramFiles` / `ProgramFiles(x86)`. `exe_parent`
/// (the running Engine binary's own directory) and its parent are included
/// too, so dev/test fixtures that co-locate everything in one folder still
/// resolve — this mirrors the historic `resolve_cortex_executable` fallback.
pub fn install_root_candidates(exe_parent: Option<&Path>, env_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(parent) = exe_parent {
        roots.push(parent.to_path_buf());
        if let Some(app_dir) = parent.parent() {
            roots.push(app_dir.to_path_buf());
        }
    }
    for root in env_roots {
        roots.push(root.join("Programs").join("Kosmos"));
        roots.push(root.join("Kosmos"));
    }
    roots
}

/// `<root>\Kosmos.exe` for every candidate root.
pub fn cortex_executable_candidates(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots.iter().map(|root| root.join("Kosmos.exe")).collect()
}

/// `<root>\resources\components\<name>\Kosmos <Name>.exe` for every
/// candidate root, matching `resolvePackagedManagerExecutable` /
/// `resolvePackagedAgendaExecutable` / `resolvePackagedMemoriaExecutable`.
pub fn component_executable_candidates(roots: &[PathBuf], component: Component) -> Vec<PathBuf> {
    roots
        .iter()
        .map(|root| {
            root.join("resources")
                .join("components")
                .join(component.dir_name())
                .join(component.exe_name())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_roots_include_exe_parent_and_app_dir() {
        let exe_parent = PathBuf::from(r"C:\Kosmos\Engine\versions\0.1.3");
        let roots = install_root_candidates(Some(&exe_parent), &[]);
        assert_eq!(
            roots,
            vec![
                PathBuf::from(r"C:\Kosmos\Engine\versions\0.1.3"),
                PathBuf::from(r"C:\Kosmos\Engine\versions"),
            ]
        );
    }

    #[test]
    fn install_roots_expand_each_env_root_two_ways() {
        let env_roots = vec![PathBuf::from(r"C:\Users\kirill\AppData\Local")];
        let roots = install_root_candidates(None, &env_roots);
        assert_eq!(
            roots,
            vec![
                PathBuf::from(r"C:\Users\kirill\AppData\Local\Programs\Kosmos"),
                PathBuf::from(r"C:\Users\kirill\AppData\Local\Kosmos"),
            ]
        );
    }

    #[test]
    fn cortex_candidates_append_the_executable_name() {
        let roots = vec![PathBuf::from(r"C:\Kosmos")];
        assert_eq!(
            cortex_executable_candidates(&roots),
            vec![PathBuf::from(r"C:\Kosmos\Kosmos.exe")]
        );
    }

    #[test]
    fn component_candidates_use_the_resources_components_layout() {
        let roots = vec![PathBuf::from(r"C:\Kosmos")];
        let candidates = component_executable_candidates(&roots, Component::Manager);
        assert_eq!(
            candidates,
            vec![PathBuf::from(
                r"C:\Kosmos\resources\components\manager\Kosmos Manager.exe"
            )]
        );
    }

    #[test]
    fn every_component_resolves_to_a_distinct_relative_path() {
        let roots = vec![PathBuf::from(r"C:\Kosmos")];
        let mut relative: Vec<PathBuf> = Component::ALL
            .iter()
            .flat_map(|component| component_executable_candidates(&roots, *component))
            .collect();
        relative.sort();
        relative.dedup();
        assert_eq!(relative.len(), Component::ALL.len());
    }
}
