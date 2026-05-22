// File Index v1 — host-local filename/path search for Kepler launcher.
//
// Storage lives in `<data_dir>/file-index.db`, not ARK. File paths are tied to
// this machine and the index can be rebuilt from disk.

mod scanner;
mod store;
mod watcher;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum FileIndexError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("file not found: {0}")]
    MissingFile(String),
    #[error("open failed: {0}")]
    Open(String),
}

pub type Result<T> = std::result::Result<T, FileIndexError>;

#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub path: String,
    pub name: String,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResult {
    pub path: String,
    pub name: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScanStats {
    pub total: usize,
    pub roots: usize,
    pub noisy_folders_excluded: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileIndexSettings {
    pub exclude_noisy_folders: bool,
    pub roots: Vec<String>,
}

pub struct FileIndex {
    roots: Vec<PathBuf>,
    store: Arc<store::FileStore>,
    scan_lock: Mutex<()>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl FileIndex {
    pub fn new(data_dir: &Path) -> Result<Self> {
        Self::with_roots(data_dir, scanner::default_roots())
    }

    pub fn with_roots(data_dir: &Path, roots: Vec<PathBuf>) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let store = Arc::new(store::FileStore::open(&data_dir.join("file-index.db"))?);
        let watcher = watcher::start(&roots, store.clone());
        Ok(Self {
            roots,
            store,
            scan_lock: Mutex::new(()),
            _watcher: watcher,
        })
    }

    pub fn settings(&self) -> Result<FileIndexSettings> {
        Ok(FileIndexSettings {
            exclude_noisy_folders: self.store.exclude_noisy_folders()?,
            roots: self
                .roots
                .iter()
                .map(|root| root.to_string_lossy().into_owned())
                .collect(),
        })
    }

    pub async fn set_exclude_noisy_folders(&self, exclude: bool) -> Result<ScanStats> {
        self.store.set_exclude_noisy_folders(exclude)?;
        self.rescan().await
    }

    pub async fn rescan(&self) -> Result<ScanStats> {
        let _guard = self.scan_lock.lock().await;
        let exclude_noisy = self.store.exclude_noisy_folders()?;
        let files = scanner::scan_roots(&self.roots, exclude_noisy);
        let total = files.len();
        self.store.replace_all(&files)?;
        Ok(ScanStats {
            total,
            roots: self.roots.len(),
            noisy_folders_excluded: exclude_noisy,
        })
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<FileSearchResult>> {
        let candidate_limit = limit.saturating_mul(64).max(512);
        let candidates = self.store.search(query, candidate_limit)?;
        Ok(rank(candidates, query, limit))
    }

    pub fn open(&self, path: &str) -> Result<()> {
        if !std::path::Path::new(path).is_file() {
            return Err(FileIndexError::MissingFile(path.to_string()));
        }
        scanner::open_file(path)
    }
}

fn rank(files: Vec<IndexedFile>, query: &str, limit: usize) -> Vec<FileSearchResult> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<FileSearchResult> = files
        .into_iter()
        .filter_map(|file| {
            let display_name = display_name(&file);
            let name = display_name.to_lowercase();
            let path = file.path.to_lowercase();
            let score = if name.starts_with(&q) {
                1.0
            } else if word_prefix(&name, &q) {
                0.82
            } else if name.contains(&q) {
                0.68
            } else if path.contains(&q) {
                0.35
            } else {
                return None;
            };
            Some(FileSearchResult {
                path: file.path,
                name: display_name,
                score,
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.path.cmp(&b.path))
    });
    out.truncate(limit);
    out
}

fn display_name(file: &IndexedFile) -> String {
    let stored = file.name.trim();
    if !stored.is_empty() {
        return stored.to_string();
    }
    file.path
        .rsplit(['\\', '/'])
        .find(|part| !part.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "Без названия".to_string())
}

fn word_prefix(name: &str, query: &str) -> bool {
    [' ', '-', '_', '.', '/', '\\']
        .iter()
        .any(|sep| name.split(*sep).any(|part| part.starts_with(query)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn scan_searches_regular_files_and_skips_noisy_folders_by_default() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("roadmap.txt"), "v1").unwrap();
        std::fs::create_dir_all(root.path().join("node_modules").join("pkg")).unwrap();
        std::fs::write(root.path().join("node_modules/pkg/roadmap-noise.txt"), "v1").unwrap();
        std::fs::create_dir_all(root.path().join(".venv").join("Lib")).unwrap();
        std::fs::write(root.path().join(".venv/Lib/roadmap-site-package.py"), "v1").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        let stats = index.rescan().await.unwrap();
        let found = index.search("roadmap", 10).unwrap();

        assert_eq!(stats.total, 1);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "roadmap.txt");
    }

    #[tokio::test]
    async fn noisy_folders_can_be_included_and_setting_persists() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".git")).unwrap();
        std::fs::write(root.path().join(".git/index-note.txt"), "v1").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        index.set_exclude_noisy_folders(false).await.unwrap();
        assert_eq!(index.search("index-note", 10).unwrap().len(), 1);

        let reopened = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        assert!(!reopened.settings().unwrap().exclude_noisy_folders);
    }

    #[tokio::test]
    async fn blank_stored_names_fall_back_to_the_path_filename() {
        let data = tempdir().unwrap();
        let index = FileIndex::with_roots(data.path(), Vec::new()).unwrap();
        index
            .store
            .upsert(&IndexedFile {
                path: r"D:\docs\nameless-plan.md".to_string(),
                name: String::new(),
                mtime: 1,
            })
            .unwrap();

        let found = index.search("nameless", 8).unwrap();

        assert_eq!(found[0].name, "nameless-plan.md");
    }

    #[tokio::test]
    async fn word_prefix_filename_matches_survive_the_candidate_window() {
        let data = tempdir().unwrap();
        let index = FileIndex::with_roots(data.path(), Vec::new()).unwrap();
        for n in 0..72 {
            index
                .store
                .upsert(&IndexedFile {
                    path: format!(r"D:\docs\areport-{n}.txt"),
                    name: format!("areport-{n}.txt"),
                    mtime: n,
                })
                .unwrap();
        }
        index
            .store
            .upsert(&IndexedFile {
                path: r"D:\docs\weekly-report-final.txt".to_string(),
                name: "weekly-report-final.txt".to_string(),
                mtime: 100,
            })
            .unwrap();

        let found = index.search("report", 1).unwrap();

        assert_eq!(found[0].name, "weekly-report-final.txt");
    }
}
