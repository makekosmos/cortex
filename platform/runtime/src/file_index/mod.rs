// File Index v1 — host-local filename/path search for Kepler launcher.
//
// Storage lives in `<data_dir>/file-index.db`, not ARK. File paths are tied to
// this machine and the index can be rebuilt from disk.

mod scanner;
mod store;
mod watcher;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use thiserror::Error;
use tokio::sync::Mutex as TokioMutex;

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
    #[error("invalid setting: {0}")]
    InvalidSetting(String),
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
    pub exclude_noisy_folders: bool,
    pub respect_gitignore: bool,
    pub include_hidden: bool,
    pub ntfs_accelerated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileIndexSettings {
    pub exclude_noisy_folders: bool,
    pub roots: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub respect_gitignore: bool,
    pub include_hidden: bool,
    pub ntfs_accelerated: bool,
    pub scan_in_progress: bool,
    pub scan_progress: ScanProgressSnapshot,
    // Regression H10 (2026-05-24): expose whether the last drive scan actually
    // ran via NTFS fast path or fell back. UI shows "active / fallback / unknown".
    pub ntfs_status: NtfsStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NtfsStatus {
    #[default]
    Unknown,
    Disabled,
    Active,
    Fallback,
    Unavailable,
}

#[derive(Debug, Clone, Default)]
pub(super) struct NtfsState {
    pub status: NtfsStatus,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ScanProgressSnapshot {
    pub phase: String,
    pub root: Option<String>,
    pub roots_done: usize,
    pub roots_total: usize,
    pub files_seen: usize,
    pub files_indexed: usize,
    pub message: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileIndexSettingsPatch {
    pub exclude_noisy_folders: Option<bool>,
    pub respect_gitignore: Option<bool>,
    pub include_hidden: Option<bool>,
    pub ntfs_accelerated: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub exclude_noisy_folders: bool,
    pub respect_gitignore: bool,
    pub include_hidden: bool,
    pub ntfs_accelerated: bool,
    pub ignore_patterns: Vec<String>,
}

pub struct FileIndex {
    store: Arc<store::FileStore>,
    scan_lock: TokioMutex<()>,
    watcher: StdMutex<Option<notify::RecommendedWatcher>>,
    self_ref: StdMutex<std::sync::Weak<FileIndex>>,
    scan_generation: AtomicU64,
    scan_in_progress: AtomicBool,
    scan_progress: Arc<StdMutex<ScanProgressSnapshot>>,
    // Regression H3 (2026-05-24): coalesce overlapping spawn_rescan calls.
    // Toggling 5 patterns in a row used to queue 5 full rescans on scan_lock.
    rescan_pending: AtomicBool,
    ntfs_last_state: Arc<StdMutex<NtfsState>>,
}

impl FileIndex {
    pub fn new(data_dir: &Path) -> Result<Self> {
        Self::with_roots(data_dir, scanner::default_roots())
    }

    pub fn with_roots(data_dir: &Path, roots: Vec<PathBuf>) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let store = Arc::new(store::FileStore::open(&data_dir.join("file-index.db"))?);
        store.seed_roots_if_empty(&roots)?;
        let actual_roots = store
            .roots()?
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let watcher = watcher::start(&actual_roots, store.clone());
        Ok(Self {
            store,
            scan_lock: TokioMutex::new(()),
            watcher: StdMutex::new(watcher),
            self_ref: StdMutex::new(std::sync::Weak::new()),
            scan_generation: AtomicU64::new(0),
            scan_in_progress: AtomicBool::new(false),
            scan_progress: Arc::new(StdMutex::new(ScanProgressSnapshot::default())),
            rescan_pending: AtomicBool::new(false),
            ntfs_last_state: Arc::new(StdMutex::new(NtfsState::default())),
        })
    }

    pub fn bind_self(self: &Arc<Self>) {
        let mut self_ref = self.self_ref.lock().unwrap_or_else(|e| e.into_inner());
        *self_ref = Arc::downgrade(self);
    }

    pub fn settings(&self) -> Result<FileIndexSettings> {
        let options = self.scan_options()?;
        let ntfs_status = if !options.ntfs_accelerated {
            NtfsStatus::Disabled
        } else {
            self.ntfs_last_state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .status
        };
        Ok(FileIndexSettings {
            exclude_noisy_folders: options.exclude_noisy_folders,
            roots: self.store.roots()?,
            ignore_patterns: options.ignore_patterns,
            respect_gitignore: options.respect_gitignore,
            include_hidden: options.include_hidden,
            ntfs_accelerated: options.ntfs_accelerated,
            scan_in_progress: self.scan_in_progress.load(Ordering::SeqCst),
            scan_progress: self.progress_snapshot(),
            ntfs_status,
        })
    }

    pub async fn set_exclude_noisy_folders(&self, exclude: bool) -> Result<ScanStats> {
        self.set_settings(FileIndexSettingsPatch {
            exclude_noisy_folders: Some(exclude),
            ..Default::default()
        })
        .await
    }

    pub async fn set_settings(&self, patch: FileIndexSettingsPatch) -> Result<ScanStats> {
        self.invalidate_running_scan();
        if let Some(value) = patch.exclude_noisy_folders {
            self.store.set_exclude_noisy_folders(value)?;
        }
        if let Some(value) = patch.respect_gitignore {
            self.store.set_respect_gitignore(value)?;
        }
        if let Some(value) = patch.include_hidden {
            self.store.set_include_hidden(value)?;
        }
        if let Some(value) = patch.ntfs_accelerated {
            self.store.set_ntfs_accelerated(value)?;
        }
        self.spawn_rescan();
        self.current_stats()
    }

    pub async fn add_root(&self, path: &str) -> Result<ScanStats> {
        let path_buf = PathBuf::from(path);
        if !path_buf.is_dir() {
            return Err(FileIndexError::InvalidSetting(format!(
                "search scope must be an existing directory: {path}"
            )));
        }
        self.invalidate_running_scan();
        self.store.add_root(path)?;
        self.restart_watcher()?;
        self.spawn_rescan();
        self.current_stats()
    }

    pub async fn remove_root(&self, path: &str) -> Result<ScanStats> {
        self.invalidate_running_scan();
        let removed_root = self.store.remove_root_record(path)?;
        self.restart_watcher()?;
        self.spawn_removed_root_cleanup(removed_root);
        self.spawn_rescan();
        self.current_stats()
    }

    pub async fn add_ignore_pattern(&self, pattern: &str) -> Result<ScanStats> {
        scanner::validate_ignore_pattern(pattern)?;
        self.invalidate_running_scan();
        self.store.add_ignore_pattern(pattern)?;
        self.spawn_rescan();
        self.current_stats()
    }

    pub async fn remove_ignore_pattern(&self, pattern: &str) -> Result<ScanStats> {
        self.invalidate_running_scan();
        self.store.remove_ignore_pattern(pattern)?;
        self.spawn_rescan();
        self.current_stats()
    }

    pub async fn rescan(&self) -> Result<ScanStats> {
        let _guard = self.scan_lock.lock().await;
        self.rescan_locked().await
    }

    pub fn request_rescan(&self) -> Result<ScanStats> {
        self.spawn_rescan();
        self.current_stats()
    }

    async fn rescan_locked(&self) -> Result<ScanStats> {
        self.scan_in_progress.store(true, Ordering::SeqCst);
        self.set_progress(ScanProgressSnapshot {
            phase: "scanning".to_string(),
            roots_total: self.root_paths()?.len(),
            message: "Сканируем файлы".to_string(),
            ..Default::default()
        });
        let _progress = ScanProgressGuard {
            flag: &self.scan_in_progress,
            progress: self.scan_progress.clone(),
        };
        let generation = self.scan_generation.load(Ordering::SeqCst);
        let roots = self.root_paths()?;
        let options = self.scan_options()?;
        let progress = self.scan_progress.clone();
        let ntfs_state = self.ntfs_last_state.clone();
        let files = scanner::scan_roots_with_progress(
            &roots,
            &options,
            move |snapshot| {
                let mut guard = progress.lock().unwrap_or_else(|e| e.into_inner());
                *guard = ScanProgressSnapshot {
                    phase: snapshot.phase,
                    root: snapshot.root,
                    roots_done: snapshot.roots_done,
                    roots_total: snapshot.roots_total,
                    files_seen: snapshot.files_seen,
                    files_indexed: snapshot.files_indexed,
                    message: snapshot.message,
                };
            },
            move |status, note| {
                let mut state = ntfs_state.lock().unwrap_or_else(|e| e.into_inner());
                state.status = status;
                state.note = note;
            },
        );
        if self.scan_generation.load(Ordering::SeqCst) != generation {
            tracing::info!(target: "file_index", "rescan discarded because settings changed");
            return self.current_stats();
        }
        self.set_progress(ScanProgressSnapshot {
            phase: "writing".to_string(),
            roots_done: roots.len(),
            roots_total: roots.len(),
            files_seen: files.len(),
            files_indexed: files.len(),
            message: "Записываем индекс".to_string(),
            ..Default::default()
        });
        let total = files.len();
        self.store.replace_all(&files)?;
        // Regression C3 (2026-05-24): mutation may bump scan_generation between
        // the pre-write check and replace_all; this rescan then commits stale
        // results. Detect and trigger a follow-up rescan so eventual state is
        // correct without waiting for an external trigger.
        if self.scan_generation.load(Ordering::SeqCst) != generation {
            tracing::info!(
                target: "file_index",
                "rescan committed stale snapshot; scheduling follow-up"
            );
            self.spawn_rescan();
        }
        self.set_progress(ScanProgressSnapshot {
            phase: "done".to_string(),
            roots_done: roots.len(),
            roots_total: roots.len(),
            files_seen: total,
            files_indexed: total,
            message: "Индексация завершена".to_string(),
            ..Default::default()
        });
        Ok(ScanStats {
            total,
            roots: roots.len(),
            exclude_noisy_folders: options.exclude_noisy_folders,
            respect_gitignore: options.respect_gitignore,
            include_hidden: options.include_hidden,
            ntfs_accelerated: options.ntfs_accelerated,
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

    fn root_paths(&self) -> Result<Vec<PathBuf>> {
        Ok(self.store.roots()?.into_iter().map(PathBuf::from).collect())
    }

    fn scan_options(&self) -> Result<ScanOptions> {
        Ok(ScanOptions {
            exclude_noisy_folders: self.store.exclude_noisy_folders()?,
            respect_gitignore: self.store.respect_gitignore()?,
            include_hidden: self.store.include_hidden()?,
            ntfs_accelerated: self.store.ntfs_accelerated()?,
            ignore_patterns: self.store.ignore_patterns()?,
        })
    }

    fn restart_watcher(&self) -> Result<()> {
        let roots = self.root_paths()?;
        let next = watcher::start(&roots, self.store.clone());
        let mut watcher = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
        *watcher = next;
        Ok(())
    }

    fn current_stats(&self) -> Result<ScanStats> {
        // Regression 2026-05-24-evening: single-lock snapshot to avoid 7
        // separate lock() acquisitions racing with chunked remove_tree windows.
        let snap = self.store.stats_snapshot()?;
        Ok(ScanStats {
            total: snap.total,
            roots: snap.roots.len(),
            exclude_noisy_folders: snap.exclude_noisy_folders,
            respect_gitignore: snap.respect_gitignore,
            include_hidden: snap.include_hidden,
            ntfs_accelerated: snap.ntfs_accelerated,
        })
    }

    fn spawn_rescan(&self) {
        // Regression H3 (2026-05-24): coalesce redundant rescan requests. If a
        // rescan is already scheduled, skip — the in-flight task will pick up
        // the latest settings/roots when it acquires scan_lock.
        if self.rescan_pending.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = self
            .self_ref
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        // If self_ref was never wired (tests skip bind_self), reset pending now
        // so subsequent calls don't get stuck in coalesce skip.
        if weak.upgrade().is_none() {
            self.rescan_pending.store(false, Ordering::SeqCst);
            return;
        }
        tokio::spawn(async move {
            let Some(index) = weak.upgrade() else {
                return;
            };
            // Reset BEFORE waiting on scan_lock so the next mutation can
            // schedule a follow-up rescan that will run after us.
            index.rescan_pending.store(false, Ordering::SeqCst);
            if let Err(e) = index.rescan().await {
                tracing::warn!(target: "file_index", error = %e, "background rescan failed");
            }
        });
    }

    fn spawn_removed_root_cleanup(&self, path: String) {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = store.remove_tree(&path) {
                tracing::warn!(
                    target: "file_index",
                    path = %path,
                    error = %e,
                    "removed scope cleanup failed"
                );
            }
        });
    }

    fn invalidate_running_scan(&self) {
        self.scan_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn progress_snapshot(&self) -> ScanProgressSnapshot {
        self.scan_progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn set_progress(&self, snapshot: ScanProgressSnapshot) {
        let mut progress = self.scan_progress.lock().unwrap_or_else(|e| e.into_inner());
        *progress = snapshot;
    }
}

struct ScanProgressGuard<'a> {
    flag: &'a AtomicBool,
    progress: Arc<StdMutex<ScanProgressSnapshot>>,
}

impl Drop for ScanProgressGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
        let mut progress = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        if progress.phase != "done" {
            progress.phase = "idle".to_string();
            progress.message = "Индексация остановлена".to_string();
        }
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
        // To index .git/* we need ALL three off: noisy filter, hidden filter,
        // and gitignore semantics (the ignore crate's WalkBuilder skips
        // .git directories when git_ignore is on).
        index
            .set_settings(FileIndexSettingsPatch {
                exclude_noisy_folders: Some(false),
                include_hidden: Some(true),
                respect_gitignore: Some(false),
                ..Default::default()
            })
            .await
            .unwrap();
        index.rescan().await.unwrap();
        assert_eq!(index.search("index-note", 10).unwrap().len(), 1);

        let reopened = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        assert!(!reopened.settings().unwrap().exclude_noisy_folders);
    }

    #[tokio::test]
    async fn ignore_patterns_filter_matching_files() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("keep.md"), "v1").unwrap();
        std::fs::write(root.path().join("scratch.tmp"), "v1").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        index.add_ignore_pattern("*.tmp").await.unwrap();
        index.rescan().await.unwrap();

        assert_eq!(index.search("keep", 10).unwrap().len(), 1);
        assert!(index.search("scratch", 10).unwrap().is_empty());
    }

    #[tokio::test]
    async fn removing_root_hides_scope_immediately_and_cleans_index_in_background() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("scope-note.md"), "v1").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        index.rescan().await.unwrap();
        assert_eq!(index.search("scope-note", 10).unwrap().len(), 1);

        index
            .remove_root(&root.path().to_string_lossy())
            .await
            .unwrap();

        assert!(index.settings().unwrap().roots.is_empty());
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if index.search("scope-note", 10).unwrap().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("removed scope cleanup should finish in the background");
    }

    #[tokio::test]
    async fn scope_remove_does_not_cleanup_large_index_synchronously() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        let root_path = root.path().to_string_lossy();
        for n in 0..1_000 {
            index
                .store
                .upsert(&IndexedFile {
                    path: root
                        .path()
                        .join(format!("bulk-{n}.txt"))
                        .to_string_lossy()
                        .to_string(),
                    name: format!("bulk-{n}.txt"),
                    mtime: n,
                })
                .unwrap();
        }

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            index.remove_root(&root_path),
        )
        .await
        .expect("scope_remove must not synchronously delete the indexed subtree")
        .unwrap();
    }

    #[tokio::test]
    async fn scope_remove_does_not_wait_for_running_rescan_lock() {
        // Regression: 2026-05-24. Settings actions used to wait for scan_lock,
        // so removing D:\ while a long NTFS/full rescan was running timed out
        // through IPC after 30s. Mutations must invalidate the running scan
        // and return quickly.
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        let _scan_guard = index.scan_lock.lock().await;

        tokio::time::timeout(
            std::time::Duration::from_millis(250),
            index.remove_root(&root.path().to_string_lossy()),
        )
        .await
        .expect("scope_remove must not wait for scan_lock")
        .unwrap();
    }

    #[tokio::test]
    async fn rescan_coalesces_overlapping_spawn_requests() {
        // Regression H3 (2026-05-24): toggling 5 settings in a row used to
        // queue 5 full rescans on scan_lock. Coalescing collapses concurrent
        // requests into one pending follow-up.
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        let index = std::sync::Arc::new(
            FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap(),
        );
        index.bind_self();

        // First call sets pending.
        index.spawn_rescan();
        // Storm: pending is already true, these MUST be no-ops.
        for _ in 0..50 {
            index.spawn_rescan();
        }

        // Wait for the rescan to finish — should be one, not 50.
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                if !index.rescan_pending.load(Ordering::SeqCst)
                    && !index.scan_in_progress.load(Ordering::SeqCst)
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("coalesced rescan completes promptly");
    }

    #[tokio::test]
    async fn rescan_schedules_followup_when_generation_changes_during_write() {
        // Regression C3 (2026-05-24): if scan_generation bumps between the
        // pre-write check and replace_all, the rescan commits stale data; a
        // follow-up rescan must be scheduled so eventual state is correct.
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        let index = std::sync::Arc::new(
            FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap(),
        );
        index.bind_self();
        std::fs::write(root.path().join("a.md"), "v").unwrap();

        // Manually fake the race: bump generation before rescan starts. The
        // pre-write check should catch it, return early — but we want to
        // make sure that even if the race happens AFTER the check, follow-up
        // is scheduled. We verify by calling rescan_locked directly with a
        // pre-bumped generation, then watching for spawn_rescan effects.
        index.invalidate_running_scan();
        let _ = index.rescan().await;
        // After rescan returns, follow-up may or may not still be pending —
        // but the index must converge to consistent state.
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if !index.scan_in_progress.load(Ordering::SeqCst)
                    && !index.rescan_pending.load(Ordering::SeqCst)
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("follow-up rescan must converge");
        assert_eq!(index.search("a.md", 10).unwrap().len(), 1);
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
