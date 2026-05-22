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
    let exclude_noisy = store.exclude_noisy_folders().unwrap_or(true);
    for path in event.paths {
        apply_path(&path, exclude_noisy, store);
    }
}

fn apply_path(path: &Path, exclude_noisy: bool, store: &FileStore) {
    let raw_path = path.to_string_lossy().into_owned();
    if exclude_noisy && scanner::path_contains_noisy_folder(path) {
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
