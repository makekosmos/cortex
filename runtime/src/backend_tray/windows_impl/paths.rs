//! Pure path-building logic for locating the packaged Manager component
//! alongside the Engine install. Kept free of `std::env` and filesystem
//! access so the candidate lists can be tested without touching a real
//! machine; [`super::resolve`] does the impure env/filesystem work.

use std::path::{Path, PathBuf};

/// Directories that may hold the packaged application (Manager under
/// `resources/components/manager`). Mirrors the NSIS per-user install under
/// `%LOCALAPPDATA%\Programs\Mundus`, or a plain `<root>\Mundus` layout under
/// any of `LOCALAPPDATA` / `ProgramFiles` / `ProgramFiles(x86)`.
/// `exe_parent` (the running Engine binary's own directory) and its parent
/// are included too, so dev/test fixtures that co-locate everything in one
/// folder still resolve.
pub fn install_root_candidates(exe_parent: Option<&Path>, env_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(parent) = exe_parent {
        roots.push(parent.to_path_buf());
        if let Some(app_dir) = parent.parent() {
            roots.push(app_dir.to_path_buf());
        }
    }
    for root in env_roots {
        roots.push(root.join("Programs").join("Mundus"));
        roots.push(root.join("Mundus"));
    }
    roots
}

/// `<root>\resources\components\manager\Mundus Manager.exe` for every
/// candidate root — Manager is the only bundled component left.
pub fn manager_executable_candidates(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .map(|root| {
            root.join("resources")
                .join("components")
                .join("manager")
                .join("Mundus Manager.exe")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_roots_include_exe_parent_and_app_dir() {
        let exe_parent = PathBuf::from(r"C:\Mundus\Engine\versions\0.1.3");
        let roots = install_root_candidates(Some(&exe_parent), &[]);
        assert_eq!(
            roots,
            vec![
                PathBuf::from(r"C:\Mundus\Engine\versions\0.1.3"),
                PathBuf::from(r"C:\Mundus\Engine\versions"),
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
                PathBuf::from(r"C:\Users\kirill\AppData\Local\Programs\Mundus"),
                PathBuf::from(r"C:\Users\kirill\AppData\Local\Mundus"),
            ]
        );
    }

    #[test]
    fn manager_candidates_use_the_resources_components_layout() {
        let roots = vec![PathBuf::from(r"C:\Mundus")];
        let candidates = manager_executable_candidates(&roots);
        assert_eq!(
            candidates,
            vec![PathBuf::from(
                r"C:\Mundus\resources\components\manager\Mundus Manager.exe"
            )]
        );
    }
}
