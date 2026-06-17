// macOS .app bundle source.
//
// Phase 1 adapter-first MVP: scan common application directories and launch
// bundles through `/usr/bin/open`. Deeper Spotlight/LaunchServices metadata and
// icon extraction can be added inside this adapter without changing callers.

use crate::app_index::app::{App, AppKind};
use crate::app_index::{AppIndexError, AppSource, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

pub fn sources() -> Vec<Box<dyn AppSource>> {
    vec![Box::new(MacApplicationsSource)]
}

pub fn launch(app: &App) -> Result<()> {
    if app.kind != AppKind::MacBundle {
        return Err(AppIndexError::Launch(format!(
            "unsupported AppKind on macOS: {:?}",
            app.kind
        )));
    }

    Command::new("/usr/bin/open")
        .arg(&app.exec_path)
        .spawn()
        .map_err(|e| AppIndexError::Launch(format!("{e}")))?;
    Ok(())
}

struct MacApplicationsSource;

impl AppSource for MacApplicationsSource {
    fn name(&self) -> &'static str {
        "mac_applications"
    }

    fn discover(&self) -> Result<Vec<App>> {
        let mut by_id = HashMap::new();
        for root in application_roots() {
            if !root.exists() {
                continue;
            }
            scan_root(&root, &mut by_id);
        }

        let mut apps: Vec<App> = by_id.into_values().collect();
        apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(apps)
    }
}

fn application_roots() -> Vec<PathBuf> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Library/CoreServices"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    roots
}

fn scan_root(root: &Path, by_id: &mut HashMap<String, App>) {
    for entry in WalkDir::new(root)
        .max_depth(3)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("app") {
            continue;
        }
        if let Some(app) = app_from_bundle(path) {
            by_id.entry(app.id.clone()).or_insert(app);
        }
    }
}

fn app_from_bundle(path: &Path) -> Option<App> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let exec_path = canonical.to_string_lossy().to_string();
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let mtime = path
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    Some(App {
        id: stable_id(&exec_path),
        name,
        exec_path,
        icon_path: None,
        icon_source: None,
        kind: AppKind::MacBundle,
        source: "mac_applications".into(),
        mtime,
    })
}

fn stable_id(exec_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(exec_path.as_bytes());
    let hash = hasher.finalize();
    let mut out = String::with_capacity(16);
    for b in &hash[..8] {
        out.push_str(&format!("{:02x}", b));
    }
    format!("mac_bundle:{out}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn stable_ids_are_prefixed_and_stable() {
        let a = stable_id("/Applications/Foo.app");
        let b = stable_id("/Applications/Foo.app");
        assert_eq!(a, b);
        assert!(a.starts_with("mac_bundle:"));
    }

    #[test]
    fn app_from_bundle_uses_bundle_name_and_path() {
        let dir = tempdir().unwrap();
        let bundle = dir.path().join("Example App.app");
        std::fs::create_dir(&bundle).unwrap();

        let app = app_from_bundle(&bundle).unwrap();
        assert_eq!(app.name, "Example App");
        assert_eq!(app.kind, AppKind::MacBundle);
        assert_eq!(app.source, "mac_applications");
        assert!(app.exec_path.ends_with("Example App.app"));
        assert!(app.id.starts_with("mac_bundle:"));
    }
}
