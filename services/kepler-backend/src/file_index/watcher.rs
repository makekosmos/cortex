use super::scanner;
use super::store::FileStore;
use super::IndexedFile;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn start(roots: &[PathBuf], store: Arc<FileStore>) -> Option<RecommendedWatcher> {
    if roots.is_empty() {
        return None;
    }
    let event_store = store.clone();
    let mut watcher = match RecommendedWatcher::new(
        move |event| handle_event(event, &event_store),
        Config::default(),
    ) {
        Ok(watcher) => watcher,
        Err(e) => {
            tracing::warn!(target: "file_index", error = %e, "watcher init failed");
            return None;
        }
    };
    for root in roots {
        if let Err(e) = watcher.watch(root, RecursiveMode::Recursive) {
            tracing::warn!(
                target: "file_index",
                root = %root.to_string_lossy(),
                error = %e,
                "watch root failed"
            );
        }
    }
    Some(watcher)
}

fn handle_event(event: notify::Result<Event>, store: &FileStore) {
    let event = match event {
        Ok(event) => event,
        Err(e) => {
            tracing::warn!(target: "file_index", error = %e, "watcher event failed");
            return;
        }
    };
    let opts = match scan_options(store) {
        Ok(opts) => opts,
        Err(e) => {
            tracing::warn!(target: "file_index", error = %e, "watcher settings load failed");
            return;
        }
    };
    // Regression C1 (2026-05-24): events queued before restart_watcher may
    // originate from a now-removed scope. Re-read active roots per-event
    // and drop anything outside them — otherwise watcher re-inserts files
    // the user just removed.
    let roots = match store.roots() {
        Ok(roots) => roots.into_iter().map(PathBuf::from).collect::<Vec<_>>(),
        Err(e) => {
            tracing::warn!(target: "file_index", error = %e, "watcher roots load failed");
            return;
        }
    };
    for path in event.paths {
        apply_path(&path, &opts, &roots, store);
    }
}

fn apply_path(path: &Path, opts: &super::ScanOptions, roots: &[PathBuf], store: &FileStore) {
    let Some(owning_root) = roots.iter().find(|root| {
        path == root.as_path() || path.starts_with(root) || {
            // Case-insensitive prefix check for Windows.
            let path_norm = path.to_string_lossy().to_lowercase();
            let root_norm = root
                .to_string_lossy()
                .trim_end_matches(['\\', '/'])
                .to_lowercase();
            !root_norm.is_empty()
                && (path_norm == root_norm
                    || path_norm.starts_with(&format!("{root_norm}\\"))
                    || path_norm.starts_with(&format!("{root_norm}/")))
        }
    }) else {
        return;
    };
    let raw_path = path.to_string_lossy().into_owned();
    // Use root-relative filtering so paths under tempdirs under %TEMP% =
    // C:\Users\...\AppData\Local\Temp don't get nuked by the **/AppData/**
    // default ignore pattern. Scanner does the same via should_index_path_for_root.
    let relative = path.strip_prefix(owning_root).unwrap_or(path);
    if !scanner::should_index_with_options(relative, opts) {
        let _ = store.remove_tree(&raw_path);
        return;
    }
    if path.is_file() {
        if let Some(file) = indexed_file(path) {
            let _ = store.upsert(&file);
        }
        return;
    }
    if !path.exists() {
        let _ = store.remove_tree(&raw_path);
    }
}

/// Используется только regression-тестом C1 (2026-05-24); сохраняем чтобы
/// при будущем refactor'е watcher'а сразу ловить ре-introduce бага «events
/// queued before restart_watcher применяются к удалённому root'у».
#[allow(dead_code)]
pub(super) fn path_is_under_any_root(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| is_path_under_root(path, root))
}

#[allow(dead_code)]
fn is_path_under_root(path: &Path, root: &Path) -> bool {
    let path_norm = path.to_string_lossy().to_lowercase();
    let root_norm = root
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_lowercase();
    if root_norm.is_empty() {
        return false;
    }
    if path_norm == root_norm {
        return true;
    }
    let with_back = format!("{root_norm}\\");
    let with_fwd = format!("{root_norm}/");
    path_norm.starts_with(&with_back) || path_norm.starts_with(&with_fwd)
}

fn scan_options(store: &FileStore) -> super::Result<super::ScanOptions> {
    Ok(super::ScanOptions {
        exclude_noisy_folders: store.exclude_noisy_folders()?,
        respect_gitignore: store.respect_gitignore()?,
        include_hidden: store.include_hidden()?,
        ntfs_accelerated: store.ntfs_accelerated()?,
        ignore_patterns: store.ignore_patterns()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_outside_active_roots_are_dropped() {
        // Regression C1 (2026-05-24): events queued before restart_watcher
        // would still be applied to the store, re-inserting files from the
        // just-removed scope.
        let roots = vec![PathBuf::from(r"D:\Active")];
        assert!(path_is_under_any_root(
            Path::new(r"D:\Active\sub\note.md"),
            &roots
        ));
        assert!(!path_is_under_any_root(
            Path::new(r"D:\Removed\note.md"),
            &roots
        ));
        // Case-insensitive — Windows paths can come back from notify lowercase.
        assert!(path_is_under_any_root(
            Path::new(r"d:\active\file.txt"),
            &roots
        ));
        // No false-positive on shared prefix.
        assert!(!path_is_under_any_root(
            Path::new(r"D:\ActiveBackup\note.md"),
            &roots
        ));
    }
}

fn indexed_file(path: &Path) -> Option<IndexedFile> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let mtime = path
        .metadata()
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|mtime| mtime.as_secs() as i64)
        .unwrap_or_default();
    Some(IndexedFile {
        path: path.to_string_lossy().into_owned(),
        name,
        mtime,
    })
}
