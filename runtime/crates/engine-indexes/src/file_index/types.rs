use super::store;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
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

pub(super) const ESTIMATE_DEFAULT_IGNORE_PATTERNS: &[&str] = &[
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
    pub(super) store: Arc<store::FileStore>,
    pub(super) scan_lock: TokioMutex<()>,
    pub(super) watcher: StdMutex<Option<notify::RecommendedWatcher>>,
    pub(super) self_ref: StdMutex<std::sync::Weak<FileIndex>>,
    pub(super) scan_generation: Arc<AtomicU64>,
    pub(super) scan_in_progress: AtomicBool,
    pub(super) scan_progress: Arc<StdMutex<ScanProgressSnapshot>>,
    pub(super) last_scan_ms: AtomicU64,
    pub(super) search_count: AtomicU64,
    pub(super) like_search_count: AtomicU64,
    pub(super) query_len_histogram: StdMutex<HashMap<String, u64>>,
    // Regression H3 (2026-05-24): coalesce overlapping spawn_rescan calls.
    // Toggling 5 patterns in a row used to queue 5 full rescans on scan_lock.
    pub(super) rescan_pending: AtomicBool,
    // KOS-270: fire-and-forget background work (rescan, removed-root cleanup)
    // holds Arc<FileStore> — an open SQLite connection. Shutdown must be able
    // to await it instead of racing a caller's tempdir teardown.
    pub(super) background_tasks: StdMutex<Vec<tokio::task::JoinHandle<()>>>,
    pub(super) ntfs_last_state: Arc<StdMutex<NtfsState>>,
    pub(super) last_scan: StdMutex<Option<LastScanSnapshot>>,
    pub(super) enabled: bool,
}
