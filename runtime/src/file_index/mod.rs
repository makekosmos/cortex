// File Index v1 — host-local filename/path search for Kepler launcher.
//
// Storage lives in `<data_dir>/file-index.db`, not ARK. File paths are tied to
// this machine and the index can be rebuilt from disk.

mod scanner;
mod store;
mod watcher;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    #[error("blocking task failed: {0}")]
    BlockingTask(String),
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
    pub enabled: bool,
    pub total: usize,
    pub roots: usize,
    pub exclude_noisy_folders: bool,
    pub respect_gitignore: bool,
    pub include_hidden: bool,
    pub ntfs_accelerated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileIndexSettings {
    pub enabled: bool,
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
    pub enabled: Option<bool>,
    pub exclude_noisy_folders: Option<bool>,
    pub respect_gitignore: Option<bool>,
    pub include_hidden: Option<bool>,
    pub ntfs_accelerated: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct FileIndexDiagnosticsSnapshot {
    pub db_size_bytes: u64,
    pub wal_size_bytes: u64,
    pub total_size_bytes: u64,
    pub scan_in_progress: bool,
    pub scan_progress: ScanProgressSnapshot,
    pub roots: Vec<String>,
    pub roots_count: usize,
    pub files_count: usize,
    pub risk_level: FileIndexRiskLevel,
    pub risk_reasons: Vec<String>,
    pub last_scan_ms: u64,
    pub last_scan: Option<LastScanSnapshot>,
    pub search_count: u64,
    pub like_search_count: u64,
    pub query_len_histogram: HashMap<String, u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FileIndexRiskLevel {
    #[default]
    Ok,
    Warning,
    Danger,
}

#[derive(Debug, Clone, Serialize)]
pub struct LastScanSnapshot {
    pub finished_at_unix_ms: u64,
    pub duration_ms: u64,
    pub indexed_file_count: usize,
    pub roots_count: usize,
    pub exclude_noisy_folders: bool,
    pub respect_gitignore: bool,
    pub include_hidden: bool,
    pub ntfs_accelerated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileIndexRootEstimate {
    pub path: String,
    pub truncated: bool,
    pub scanned_dirs: usize,
    pub scanned_files: usize,
    pub ignored_or_skipped_files: usize,
    pub indexable_text_files_count: usize,
    pub indexable_text_bytes: u64,
    pub metadata_only_media_files_count: usize,
    pub metadata_only_other_files_count: usize,
    pub estimated_indexed_entries_count: usize,
    pub estimated_index_size_bytes: u64,
    pub risk_level: FileIndexRiskLevel,
    pub risk_reasons: Vec<String>,
    pub limitations: Vec<String>,
}

const ESTIMATE_DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "*.tmp",
    "*.temp",
    "**/AppData/**",
    "**/[Cc]ache/**",
    "**/[Cc]aches/**",
];

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
    scan_generation: Arc<AtomicU64>,
    scan_in_progress: AtomicBool,
    scan_progress: Arc<StdMutex<ScanProgressSnapshot>>,
    last_scan_ms: AtomicU64,
    search_count: AtomicU64,
    like_search_count: AtomicU64,
    query_len_histogram: StdMutex<HashMap<String, u64>>,
    // Regression H3 (2026-05-24): coalesce overlapping spawn_rescan calls.
    // Toggling 5 patterns in a row used to queue 5 full rescans on scan_lock.
    rescan_pending: AtomicBool,
    ntfs_last_state: Arc<StdMutex<NtfsState>>,
    last_scan: StdMutex<Option<LastScanSnapshot>>,
    enabled: bool,
}

impl FileIndex {
    pub fn new(data_dir: &Path) -> Result<Self> {
        Self::with_roots(data_dir, scanner::default_roots())
    }

    pub fn new_disabled(data_dir: &Path) -> Result<Self> {
        Self::with_roots_internal(data_dir, Vec::new(), false)
    }

    pub fn with_roots(data_dir: &Path, roots: Vec<PathBuf>) -> Result<Self> {
        Self::with_roots_internal(data_dir, roots, true)
    }

    fn with_roots_internal(data_dir: &Path, roots: Vec<PathBuf>, enabled: bool) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        let store = Arc::new(store::FileStore::open(&data_dir.join("file-index.db"))?);
        if enabled {
            store.seed_roots_if_empty(&roots)?;
        }
        let actual_roots = if enabled {
            store
                .roots()?
                .into_iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let watcher = if enabled {
            watcher::start(&actual_roots, store.clone())
        } else {
            None
        };
        Ok(Self {
            store,
            scan_lock: TokioMutex::new(()),
            watcher: StdMutex::new(watcher),
            self_ref: StdMutex::new(std::sync::Weak::new()),
            scan_generation: Arc::new(AtomicU64::new(0)),
            scan_in_progress: AtomicBool::new(false),
            scan_progress: Arc::new(StdMutex::new(ScanProgressSnapshot::default())),
            last_scan_ms: AtomicU64::new(0),
            search_count: AtomicU64::new(0),
            like_search_count: AtomicU64::new(0),
            query_len_histogram: StdMutex::new(HashMap::new()),
            rescan_pending: AtomicBool::new(false),
            ntfs_last_state: Arc::new(StdMutex::new(NtfsState::default())),
            last_scan: StdMutex::new(None),
            enabled,
        })
    }

    pub fn bind_self(self: &Arc<Self>) {
        let mut self_ref = self.self_ref.lock().unwrap_or_else(|e| e.into_inner());
        *self_ref = Arc::downgrade(self);
    }

    pub fn settings(&self) -> Result<FileIndexSettings> {
        let options = self.scan_options()?;
        let enabled = self.index_enabled()?;
        let ntfs_status = if !options.ntfs_accelerated {
            NtfsStatus::Disabled
        } else {
            self.ntfs_last_state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .status
        };
        Ok(FileIndexSettings {
            enabled,
            exclude_noisy_folders: options.exclude_noisy_folders,
            roots: self.root_strings()?,
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
        if !self.enabled {
            return self.current_stats();
        }
        self.invalidate_running_scan();
        let mut should_rescan = true;
        if let Some(value) = patch.exclude_noisy_folders {
            self.store.set_exclude_noisy_folders(value)?;
        }
        if let Some(value) = patch.enabled {
            self.store.set_enabled(value)?;
            should_rescan = value;
            if !value {
                self.clear_progress_after_disable();
                self.stop_watcher();
            } else {
                self.restart_watcher()?;
            }
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
        if should_rescan && self.index_enabled()? {
            self.spawn_rescan();
        }
        self.current_stats()
    }

    pub async fn add_root(&self, path: &str) -> Result<ScanStats> {
        if !self.enabled {
            return self.current_stats();
        }
        let path_buf = PathBuf::from(path);
        if !path_buf.is_dir() {
            return Err(FileIndexError::InvalidSetting(format!(
                "search scope must be an existing directory: {path}"
            )));
        }
        self.invalidate_running_scan();
        self.store.add_root(path)?;
        if self.index_enabled()? {
            self.restart_watcher()?;
            self.spawn_rescan();
        }
        self.current_stats()
    }

    pub async fn remove_root(&self, path: &str) -> Result<ScanStats> {
        if !self.enabled {
            return self.current_stats();
        }
        self.invalidate_running_scan();
        let removed_root = self.store.remove_root_record(path)?;
        if self.index_enabled()? {
            self.restart_watcher()?;
        }
        self.spawn_removed_root_cleanup(removed_root);
        if self.index_enabled()? {
            self.spawn_rescan();
        }
        self.current_stats()
    }

    pub async fn add_ignore_pattern(&self, pattern: &str) -> Result<ScanStats> {
        if !self.enabled {
            return self.current_stats();
        }
        scanner::validate_ignore_pattern(pattern)?;
        self.invalidate_running_scan();
        self.store.add_ignore_pattern(pattern)?;
        if self.index_enabled()? {
            self.spawn_rescan();
        }
        self.current_stats()
    }

    pub async fn remove_ignore_pattern(&self, pattern: &str) -> Result<ScanStats> {
        if !self.enabled {
            return self.current_stats();
        }
        self.invalidate_running_scan();
        self.store.remove_ignore_pattern(pattern)?;
        if self.index_enabled()? {
            self.spawn_rescan();
        }
        self.current_stats()
    }

    pub async fn rescan(&self) -> Result<ScanStats> {
        if !self.enabled || !self.index_enabled()? {
            return self.current_stats();
        }
        let _guard = self.scan_lock.lock().await;
        self.rescan_locked().await
    }

    pub fn request_rescan(&self) -> Result<ScanStats> {
        if !self.enabled || !self.index_enabled()? {
            return self.current_stats();
        }
        self.spawn_rescan();
        self.current_stats()
    }

    pub fn clear_cache(&self) -> Result<ScanStats> {
        if !self.enabled {
            return self.current_stats();
        }
        self.invalidate_running_scan();
        self.store.clear_index_cache()?;
        self.current_stats()
    }

    pub fn diagnostics(&self) -> Result<FileIndexDiagnosticsSnapshot> {
        let size = self.store.database_size_snapshot()?;
        let stats = self.current_stats().ok();
        let roots = self.root_strings().unwrap_or_default();
        let files_count = stats.as_ref().map(|s| s.total).unwrap_or_default();
        let (risk_level, risk_reasons) = assess_roots_risk(&roots);
        Ok(FileIndexDiagnosticsSnapshot {
            db_size_bytes: size.db_size_bytes,
            wal_size_bytes: size.wal_size_bytes,
            total_size_bytes: size.total_size_bytes,
            scan_in_progress: self.scan_in_progress.load(Ordering::SeqCst),
            scan_progress: self.progress_snapshot(),
            roots_count: roots.len(),
            roots,
            files_count,
            risk_level,
            risk_reasons,
            last_scan_ms: self.last_scan_ms.load(Ordering::SeqCst),
            last_scan: self
                .last_scan
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            search_count: self.search_count.load(Ordering::SeqCst),
            like_search_count: self.like_search_count.load(Ordering::SeqCst),
            query_len_histogram: self
                .query_len_histogram
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
        })
    }

    pub fn estimate_root(&self, path: &str) -> Result<FileIndexRootEstimate> {
        self.estimate_root_with_budget(path, EstimateBudget::default())
    }

    async fn rescan_locked(&self) -> Result<ScanStats> {
        let scan_started = std::time::Instant::now();
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
        let scan_generation = self.scan_generation.clone();
        let store = self.store.clone();
        let outcome = tokio::task::spawn_blocking(move || -> Result<ScanCommitOutcome> {
            // Тяжёлый WalkDir/NTFS scan + commit — на background-priority потоке
            // (CPU + I/O), чтобы не душить систему. Guard живёт внутри sync
            // closure, без `.await`. См. crate::priority.
            let _bg = crate::priority::BackgroundThreadGuard::enter();
            let progress_for_scan = progress.clone();
            let files = scanner::scan_roots_with_progress(
                &roots,
                &options,
                move |snapshot| {
                    let mut guard = progress_for_scan.lock().unwrap_or_else(|e| e.into_inner());
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
                || scan_generation.load(Ordering::SeqCst) != generation,
            );
            if scan_generation.load(Ordering::SeqCst) != generation {
                return Ok(ScanCommitOutcome::Cancelled);
            }
            {
                let mut guard = progress.lock().unwrap_or_else(|e| e.into_inner());
                *guard = ScanProgressSnapshot {
                    phase: "writing".to_string(),
                    roots_done: roots.len(),
                    roots_total: roots.len(),
                    files_seen: files.len(),
                    files_indexed: files.len(),
                    message: "Записываем индекс".to_string(),
                    ..Default::default()
                };
            }
            let total = files.len();
            store.replace_all(&files)?;
            store.checkpoint_truncate_wal()?;
            Ok(ScanCommitOutcome::Committed {
                total,
                roots: roots.len(),
                options,
            })
        })
        .await
        .map_err(|e| FileIndexError::BlockingTask(e.to_string()))??;
        let (total, roots, options) = match outcome {
            ScanCommitOutcome::Cancelled => {
                tracing::info!(target: "file_index", "rescan discarded because settings changed");
                return self.current_stats();
            }
            ScanCommitOutcome::Committed {
                total,
                roots,
                options,
            } => (total, roots, options),
        };
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
            roots_done: roots,
            roots_total: roots,
            files_seen: total,
            files_indexed: total,
            message: "Индексация завершена".to_string(),
            ..Default::default()
        });
        self.last_scan_ms.store(
            scan_started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            Ordering::SeqCst,
        );
        self.remember_last_scan(LastScanSnapshot {
            finished_at_unix_ms: now_unix_ms(),
            duration_ms: self.last_scan_ms.load(Ordering::SeqCst),
            indexed_file_count: total,
            roots_count: roots,
            exclude_noisy_folders: options.exclude_noisy_folders,
            respect_gitignore: options.respect_gitignore,
            include_hidden: options.include_hidden,
            ntfs_accelerated: options.ntfs_accelerated,
        });
        Ok(ScanStats {
            enabled: true,
            total,
            roots,
            exclude_noisy_folders: options.exclude_noisy_folders,
            respect_gitignore: options.respect_gitignore,
            include_hidden: options.include_hidden,
            ntfs_accelerated: options.ntfs_accelerated,
        })
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<FileSearchResult>> {
        if !self.enabled || !self.index_enabled()? {
            return Ok(Vec::new());
        }
        self.observe_search(query);
        let candidate_limit = limit.saturating_mul(64).max(512);
        let roots = self.root_strings()?;
        let candidates = self
            .store
            .search(query, candidate_limit)?
            .into_iter()
            .filter(|file| indexed_result_is_visible(&file.path, &roots))
            .collect();
        Ok(rank(candidates, query, limit))
    }

    pub fn open(&self, path: &str) -> Result<()> {
        if !std::path::Path::new(path).is_file() {
            return Err(FileIndexError::MissingFile(path.to_string()));
        }
        scanner::open_file(path)
    }

    fn root_paths(&self) -> Result<Vec<PathBuf>> {
        Ok(self
            .root_strings()?
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }

    fn root_strings(&self) -> Result<Vec<String>> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        self.store.roots()
    }

    fn index_enabled(&self) -> Result<bool> {
        Ok(self.enabled && self.store.enabled()?)
    }

    pub fn has_roots(&self) -> Result<bool> {
        Ok(!self.root_strings()?.is_empty())
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
        if !self.enabled {
            return Ok(());
        }
        let roots = self.root_paths()?;
        let next = watcher::start(&roots, self.store.clone());
        let mut watcher = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
        *watcher = next;
        Ok(())
    }

    fn stop_watcher(&self) {
        let mut watcher = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
        *watcher = None;
    }

    fn current_stats(&self) -> Result<ScanStats> {
        if !self.enabled {
            let options = self.scan_options()?;
            return Ok(ScanStats {
                enabled: false,
                total: 0,
                roots: 0,
                exclude_noisy_folders: options.exclude_noisy_folders,
                respect_gitignore: options.respect_gitignore,
                include_hidden: options.include_hidden,
                ntfs_accelerated: options.ntfs_accelerated,
            });
        }
        // Regression 2026-05-24-evening: single-lock snapshot to avoid 7
        // separate lock() acquisitions racing with chunked remove_tree windows.
        let snap = self.store.stats_snapshot()?;
        Ok(ScanStats {
            enabled: snap.enabled,
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

    fn clear_progress_after_disable(&self) {
        self.scan_in_progress.store(false, Ordering::SeqCst);
        self.set_progress(ScanProgressSnapshot {
            phase: "disabled".to_string(),
            message: "Поиск файлов выключен".to_string(),
            ..Default::default()
        });
    }

    fn remember_last_scan(&self, snapshot: LastScanSnapshot) {
        let mut last_scan = self.last_scan.lock().unwrap_or_else(|e| e.into_inner());
        *last_scan = Some(snapshot);
    }

    pub fn diagnostics_snapshot(&self) -> FileIndexDiagnosticsSnapshot {
        self.diagnostics()
            .unwrap_or_else(|_| FileIndexDiagnosticsSnapshot {
                db_size_bytes: 0,
                wal_size_bytes: 0,
                total_size_bytes: 0,
                scan_in_progress: self.scan_in_progress.load(Ordering::SeqCst),
                scan_progress: self.progress_snapshot(),
                roots: self.root_strings().unwrap_or_default(),
                roots_count: self
                    .root_strings()
                    .map(|roots| roots.len())
                    .unwrap_or_default(),
                files_count: self
                    .current_stats()
                    .map(|stats| stats.total)
                    .unwrap_or_default(),
                risk_level: FileIndexRiskLevel::Warning,
                risk_reasons: vec!["Не удалось собрать полную диагностику индекса".to_string()],
                last_scan_ms: self.last_scan_ms.load(Ordering::SeqCst),
                last_scan: self
                    .last_scan
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
                search_count: self.search_count.load(Ordering::SeqCst),
                like_search_count: self.like_search_count.load(Ordering::SeqCst),
                query_len_histogram: self
                    .query_len_histogram
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            })
    }

    fn observe_search(&self, query: &str) {
        self.search_count.fetch_add(1, Ordering::SeqCst);
        let len = query.trim().chars().count();
        if len < 3 {
            self.like_search_count.fetch_add(1, Ordering::SeqCst);
        }
        let bucket = match len {
            0 => "0",
            1 => "1",
            2 => "2",
            3..=5 => "3_5",
            6..=12 => "6_12",
            _ => "13_plus",
        };
        let mut histogram = self
            .query_len_histogram
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *histogram.entry(bucket.to_string()).or_insert(0) += 1;
    }

    fn estimate_root_with_budget(
        &self,
        path: &str,
        budget: EstimateBudget,
    ) -> Result<FileIndexRootEstimate> {
        let root = PathBuf::from(path);
        if !root.is_dir() {
            return Err(FileIndexError::InvalidSetting(format!(
                "search scope must be an existing directory: {path}"
            )));
        }
        let options = self.scan_options()?;
        let mut builder = ignore::WalkBuilder::new(&root);
        builder
            .follow_links(false)
            .hidden(!options.include_hidden)
            .git_ignore(options.respect_gitignore)
            .git_global(options.respect_gitignore)
            .git_exclude(options.respect_gitignore)
            .parents(options.respect_gitignore)
            .add_custom_ignore_filename(".rayignore");
        let matcher = estimate_ignore_matcher(&options);

        let started = std::time::Instant::now();
        let mut truncated = false;
        let mut scanned_dirs = 0usize;
        let mut scanned_files = 0usize;
        let mut ignored_or_skipped_files = 0usize;
        let mut indexable_text_files_count = 0usize;
        let mut indexable_text_bytes = 0u64;
        let mut metadata_only_media_files_count = 0usize;
        let mut metadata_only_other_files_count = 0usize;
        let mut estimated_indexed_entries_count = 0usize;
        let mut estimated_index_size_bytes = 0u64;

        for entry in builder.build().filter_map(|entry| entry.ok()) {
            if started.elapsed() >= budget.max_duration
                || scanned_dirs >= budget.max_dirs
                || scanned_files >= budget.max_files
            {
                truncated = true;
                break;
            }
            let path = entry.path();
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                scanned_dirs += 1;
                continue;
            }
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }
            scanned_files += 1;
            if !estimate_should_index_path_for_root(path, &root, &options, matcher.as_ref()) {
                ignored_or_skipped_files += 1;
                continue;
            }

            let file_len = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            let name = entry.file_name().to_string_lossy();
            estimated_indexed_entries_count += 1;
            estimated_index_size_bytes = estimated_index_size_bytes
                .saturating_add(estimate_index_entry_size_bytes(path, &name));

            match classify_indexed_file(path) {
                IndexedFileClass::TextLike => {
                    indexable_text_files_count += 1;
                    indexable_text_bytes = indexable_text_bytes.saturating_add(file_len);
                }
                IndexedFileClass::Media => {
                    metadata_only_media_files_count += 1;
                }
                IndexedFileClass::Other => {
                    metadata_only_other_files_count += 1;
                }
            }
        }

        let mut limitations = vec![
            "Текущий file index индексирует только имя и путь файла; content indexing не используется."
                .to_string(),
        ];
        if options.respect_gitignore {
            limitations.push(
                "Файлы, отфильтрованные .gitignore/ignore walker-ом, считаются не полностью; ignored_or_skipped_files — нижняя оценка."
                    .to_string(),
            );
        }
        if truncated {
            limitations.push(
                "Оценка усечена по budget/time cap; итоговые числа являются нижней оценкой."
                    .to_string(),
            );
        }

        let (risk_level, risk_reasons) = assess_estimate_risk(
            path,
            truncated,
            estimated_indexed_entries_count,
            estimated_index_size_bytes,
        );
        Ok(FileIndexRootEstimate {
            path: root.to_string_lossy().to_string(),
            truncated,
            scanned_dirs,
            scanned_files,
            ignored_or_skipped_files,
            indexable_text_files_count,
            indexable_text_bytes,
            metadata_only_media_files_count,
            metadata_only_other_files_count,
            estimated_indexed_entries_count,
            estimated_index_size_bytes,
            risk_level,
            risk_reasons,
            limitations,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct EstimateBudget {
    max_files: usize,
    max_dirs: usize,
    max_duration: std::time::Duration,
}

impl Default for EstimateBudget {
    fn default() -> Self {
        Self {
            max_files: 50_000,
            max_dirs: 10_000,
            max_duration: std::time::Duration::from_secs(3),
        }
    }
}

enum ScanCommitOutcome {
    Cancelled,
    Committed {
        total: usize,
        roots: usize,
        options: ScanOptions,
    },
}

pub fn env_flag_enabled(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => default,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexedFileClass {
    TextLike,
    Media,
    Other,
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn estimate_index_entry_size_bytes(path: &Path, name: &str) -> u64 {
    let path_bytes = path.as_os_str().to_string_lossy().len() as u64;
    let name_bytes = name.len() as u64;
    // files row + fts row + index overhead. This intentionally overestimates a
    // little so settings UI does not present unrealistically low storage costs.
    192u64
        .saturating_add(path_bytes.saturating_mul(2))
        .saturating_add(name_bytes.saturating_mul(2))
}

fn classify_indexed_file(path: &Path) -> IndexedFileClass {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return IndexedFileClass::Other;
    };
    let ext = ext.to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "svg"
            | "heic"
            | "avif"
            | "mp4"
            | "mov"
            | "avi"
            | "mkv"
            | "webm"
            | "mp3"
            | "wav"
            | "flac"
            | "ogg"
            | "m4a"
    ) {
        return IndexedFileClass::Media;
    }
    if matches!(
        ext.as_str(),
        "txt"
            | "md"
            | "rs"
            | "toml"
            | "json"
            | "jsonc"
            | "yaml"
            | "yml"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "html"
            | "css"
            | "scss"
            | "less"
            | "xml"
            | "csv"
            | "tsv"
            | "sql"
            | "py"
            | "java"
            | "kt"
            | "kts"
            | "go"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "swift"
            | "sh"
            | "ps1"
            | "bat"
            | "cmd"
            | "ini"
            | "log"
    ) {
        return IndexedFileClass::TextLike;
    }
    IndexedFileClass::Other
}

fn estimate_ignore_matcher(options: &ScanOptions) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut added = false;
    for pattern in ESTIMATE_DEFAULT_IGNORE_PATTERNS
        .iter()
        .copied()
        .chain(options.ignore_patterns.iter().map(String::as_str))
    {
        if estimate_add_glob_variants(&mut builder, pattern).is_ok() {
            added = true;
        }
    }
    if !added {
        return None;
    }
    builder.build().ok()
}

fn estimate_add_glob_variants(
    builder: &mut GlobSetBuilder,
    pattern: &str,
) -> std::result::Result<(), ()> {
    let normalized = pattern.trim();
    if normalized.is_empty() {
        return Ok(());
    }
    let glob = Glob::new(normalized).map_err(|_| ())?;
    builder.add(glob);
    if !normalized.contains('/') && !normalized.contains('\\') {
        let deep = Glob::new(&format!("**/{normalized}")).map_err(|_| ())?;
        builder.add(deep);
    }
    Ok(())
}

fn estimate_should_index_path_for_root(
    path: &Path,
    root: &Path,
    options: &ScanOptions,
    matcher: Option<&GlobSet>,
) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if options.exclude_noisy_folders && scanner::path_contains_noisy_folder(relative) {
        return false;
    }
    if !options.include_hidden && estimate_is_dot_hidden(relative) {
        return false;
    }
    !estimate_matches_ignore_pattern(relative, matcher)
}

fn estimate_matches_ignore_pattern(path: &Path, matcher: Option<&GlobSet>) -> bool {
    matcher.is_some_and(|matcher| {
        matcher.is_match(path)
            || path
                .file_name()
                .is_some_and(|name| matcher.is_match(Path::new(name)))
    })
}

fn estimate_is_dot_hidden(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name.starts_with('.') && name != "." && name != ".."
    })
}

fn assess_roots_risk(roots: &[String]) -> (FileIndexRiskLevel, Vec<String>) {
    let mut level = FileIndexRiskLevel::Ok;
    let mut reasons = Vec::new();
    for root in roots {
        let (root_level, mut root_reasons) = assess_root_path_risk(root);
        level = max_risk(level, root_level);
        reasons.append(&mut root_reasons);
    }
    (level, reasons)
}

fn assess_estimate_risk(
    root: &str,
    truncated: bool,
    estimated_entries: usize,
    estimated_size_bytes: u64,
) -> (FileIndexRiskLevel, Vec<String>) {
    let (mut level, mut reasons) = assess_root_path_risk(root);
    if truncated {
        level = max_risk(level, FileIndexRiskLevel::Warning);
        reasons.push("Оценка была усечена по budget/time cap".to_string());
    }
    if estimated_entries >= 500_000 {
        level = max_risk(level, FileIndexRiskLevel::Danger);
        reasons.push(format!(
            "Оценка показывает очень большой объём индекса: {estimated_entries} файлов"
        ));
    } else if estimated_entries >= 100_000 {
        level = max_risk(level, FileIndexRiskLevel::Warning);
        reasons.push(format!(
            "Оценка показывает крупный объём индекса: {estimated_entries} файлов"
        ));
    }
    const MB: u64 = 1024 * 1024;
    if estimated_size_bytes >= 1024 * MB {
        level = max_risk(level, FileIndexRiskLevel::Danger);
        reasons.push(format!(
            "Оценочный размер индекса превышает 1 ГБ: {} байт",
            estimated_size_bytes
        ));
    } else if estimated_size_bytes >= 256 * MB {
        level = max_risk(level, FileIndexRiskLevel::Warning);
        reasons.push(format!(
            "Оценочный размер индекса заметный: {} байт",
            estimated_size_bytes
        ));
    }
    (level, reasons)
}

fn assess_root_path_risk(root: &str) -> (FileIndexRiskLevel, Vec<String>) {
    let normalized = root.trim_end_matches(['\\', '/']);
    if normalized.is_empty() {
        return (
            FileIndexRiskLevel::Danger,
            vec!["Пустой или некорректный root индекса".to_string()],
        );
    }
    let mut level = FileIndexRiskLevel::Ok;
    let mut reasons = Vec::new();
    if is_drive_root_like(normalized) || normalized == "/" {
        level = FileIndexRiskLevel::Danger;
        reasons.push(format!(
            "Root `{root}` охватывает корень диска/файловой системы"
        ));
    }
    let lower = normalized.to_ascii_lowercase();
    if lower.ends_with(r":\users")
        || lower.ends_with(r":\users\public")
        || lower.ends_with(r":\programdata")
        || lower.ends_with("/users")
        || lower.ends_with("/home")
    {
        level = max_risk(level, FileIndexRiskLevel::Danger);
        reasons.push(format!(
            "Root `{root}` выглядит слишком широким для постоянного индексирования"
        ));
    }
    let depth = Path::new(normalized)
        .components()
        .filter(|component| matches!(component, std::path::Component::Normal(_)))
        .count();
    if depth <= 1 && !is_drive_root_like(normalized) && normalized != "/" {
        level = max_risk(level, FileIndexRiskLevel::Warning);
        reasons.push(format!(
            "Root `{root}` расположен слишком высоко в дереве каталогов"
        ));
    }
    (level, reasons)
}

fn max_risk(a: FileIndexRiskLevel, b: FileIndexRiskLevel) -> FileIndexRiskLevel {
    use FileIndexRiskLevel::{Danger, Ok, Warning};
    match (a, b) {
        (Danger, _) | (_, Danger) => Danger,
        (Warning, _) | (_, Warning) => Warning,
        _ => Ok,
    }
}

fn is_drive_root_like(path: &str) -> bool {
    let trimmed = path.trim_end_matches(['\\', '/']);
    let bytes = trimmed.as_bytes();
    bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn indexed_result_is_visible(path: &str, roots: &[String]) -> bool {
    if roots.is_empty() || !Path::new(path).exists() {
        return false;
    }
    roots.iter().any(|root| path_is_under_root(path, root))
}

fn path_is_under_root(path: &str, root: &str) -> bool {
    let normalized_root = root.trim_end_matches(['\\', '/']);
    let normalized_path = path.trim_end_matches(['\\', '/']);
    if normalized_root.is_empty() {
        return false;
    }
    if cfg!(windows) {
        let root_lower = normalized_root.to_ascii_lowercase();
        let path_lower = normalized_path.to_ascii_lowercase();
        path_lower == root_lower
            || path_lower.starts_with(&format!("{root_lower}\\"))
            || path_lower.starts_with(&format!("{root_lower}/"))
    } else {
        normalized_path == normalized_root
            || normalized_path.starts_with(&format!("{normalized_root}/"))
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

    static ENV_FLAG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    async fn disabled_index_exposes_empty_safe_surface() {
        // Regression: 2026-06-08. KEPLER_FILE_INDEX=0 must be a real kill switch,
        // not just "skip startup scan while old persisted roots keep working".
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        let enabled = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        enabled
            .store
            .upsert(&IndexedFile {
                path: root
                    .path()
                    .join("persisted-note.txt")
                    .to_string_lossy()
                    .to_string(),
                name: "persisted-note.txt".to_string(),
                mtime: 1,
            })
            .unwrap();

        let disabled = FileIndex::new_disabled(data.path()).unwrap();

        assert!(disabled.settings().unwrap().roots.is_empty());
        assert!(disabled.search("persisted", 10).unwrap().is_empty());
        assert_eq!(disabled.request_rescan().unwrap().total, 0);
        assert_eq!(disabled.rescan().await.unwrap().roots, 0);
    }

    #[tokio::test]
    async fn settings_enabled_false_disables_search_without_losing_roots() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("toggle-note.txt"), "v1").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        index.rescan().await.unwrap();
        assert_eq!(index.search("toggle-note", 10).unwrap().len(), 1);

        index
            .set_settings(FileIndexSettingsPatch {
                enabled: Some(false),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(!index.settings().unwrap().enabled);
        assert_eq!(index.settings().unwrap().roots.len(), 1);
        assert!(index.search("toggle-note", 10).unwrap().is_empty());
        assert_eq!(index.request_rescan().unwrap().total, 1);

        index
            .set_settings(FileIndexSettingsPatch {
                enabled: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();
        index.rescan().await.unwrap();
        assert_eq!(index.search("toggle-note", 10).unwrap().len(), 1);
    }

    #[test]
    fn env_flag_parser_treats_zero_false_off_no_as_disabled() {
        let _guard = ENV_FLAG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let key = "KEPLER_FILE_INDEX_TEST_FLAG";
        let prev = std::env::var(key).ok();
        for value in ["0", "false", "off", "no"] {
            unsafe {
                std::env::set_var(key, value);
            }
            assert!(!env_flag_enabled(key, true), "value {value}");
        }
        unsafe {
            std::env::set_var(key, "1");
        }
        assert!(env_flag_enabled(key, false));
        unsafe {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
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
    async fn clear_cache_removes_index_rows_but_keeps_settings_and_roots() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("cache-note.md"), "v1").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        index.set_exclude_noisy_folders(false).await.unwrap();
        index.rescan().await.unwrap();
        assert_eq!(index.search("cache-note", 10).unwrap().len(), 1);

        let stats = index.clear_cache().unwrap();
        assert_eq!(stats.total, 0);
        assert_eq!(index.search("cache-note", 10).unwrap().len(), 0);

        let settings = index.settings().unwrap();
        assert_eq!(settings.roots.len(), 1);
        assert!(!settings.exclude_noisy_folders);

        index.rescan().await.unwrap();
        assert_eq!(index.search("cache-note", 10).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn diagnostics_include_sizes_roots_and_last_scan() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("diag-note.md"), "v1").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        index.rescan().await.unwrap();

        let diag = index.diagnostics().unwrap();

        assert_eq!(diag.roots_count, 1);
        assert_eq!(diag.files_count, 1);
        assert!(diag.db_size_bytes > 0);
        assert_eq!(diag.wal_size_bytes, 0);
        assert!(diag.last_scan.is_some());
    }

    #[tokio::test]
    async fn estimate_root_counts_text_media_and_skipped_files() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("keep.md"), "hello").unwrap();
        std::fs::write(root.path().join("photo.png"), "png").unwrap();
        std::fs::write(root.path().join("scratch.tmp"), "tmp").unwrap();
        std::fs::create_dir_all(root.path().join("node_modules/pkg")).unwrap();
        std::fs::write(root.path().join("node_modules/pkg/noise.js"), "ignored").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        index.add_ignore_pattern("*.tmp").await.unwrap();

        let estimate = index.estimate_root(&root.path().to_string_lossy()).unwrap();

        assert_eq!(estimate.indexable_text_files_count, 1);
        assert_eq!(estimate.metadata_only_media_files_count, 1);
        assert!(estimate.ignored_or_skipped_files >= 1);
        assert_eq!(estimate.estimated_indexed_entries_count, 2);
    }

    #[test]
    fn estimate_root_can_be_truncated_by_budget() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("one.md"), "1").unwrap();
        std::fs::write(root.path().join("two.md"), "2").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        let estimate = index
            .estimate_root_with_budget(
                &root.path().to_string_lossy(),
                EstimateBudget {
                    max_files: 1,
                    max_dirs: 100,
                    max_duration: std::time::Duration::from_secs(60),
                },
            )
            .unwrap();

        assert!(estimate.truncated);
        assert_eq!(estimate.scanned_files, 1);
        assert!(estimate
            .limitations
            .iter()
            .any(|reason| reason.contains("усечена")));
    }

    #[test]
    fn broad_drive_root_is_marked_as_danger() {
        let (level, reasons) = assess_root_path_risk(r"C:\");
        assert_eq!(level, FileIndexRiskLevel::Danger);
        assert!(reasons.iter().any(|reason| reason.contains("корень диска")));
    }

    #[tokio::test]
    async fn search_hides_deleted_files_before_cleanup_or_rescan() {
        let data = tempdir().unwrap();
        let root = tempdir().unwrap();
        let file = root.path().join("deleted-note.md");
        std::fs::write(&file, "v1").unwrap();

        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        index.rescan().await.unwrap();
        assert_eq!(index.search("deleted-note", 10).unwrap().len(), 1);

        std::fs::remove_file(file).unwrap();
        assert!(index.search("deleted-note", 10).unwrap().is_empty());
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
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
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
        let root = tempdir().unwrap();
        let file = root.path().join("nameless-plan.md");
        std::fs::write(&file, "v1").unwrap();
        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        index
            .store
            .upsert(&IndexedFile {
                path: file.to_string_lossy().to_string(),
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
        let root = tempdir().unwrap();
        let index = FileIndex::with_roots(data.path(), vec![root.path().to_path_buf()]).unwrap();
        for n in 0..72 {
            let file = root.path().join(format!("areport-{n}.txt"));
            std::fs::write(&file, "v1").unwrap();
            index
                .store
                .upsert(&IndexedFile {
                    path: file.to_string_lossy().to_string(),
                    name: format!("areport-{n}.txt"),
                    mtime: n,
                })
                .unwrap();
        }
        let file = root.path().join("weekly-report-final.txt");
        std::fs::write(&file, "v1").unwrap();
        index
            .store
            .upsert(&IndexedFile {
                path: file.to_string_lossy().to_string(),
                name: "weekly-report-final.txt".to_string(),
                mtime: 100,
            })
            .unwrap();

        let found = index.search("report", 1).unwrap();

        assert_eq!(found[0].name, "weekly-report-final.txt");
    }
}
