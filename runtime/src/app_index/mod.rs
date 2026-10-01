// App Index — поиск и запуск установленных приложений (2026-05-22).
//
// Cross-platform-ready: trait `AppSource` + per-platform impl в `platform/`.
// Windows v1 sources: Start Menu (.lnk) + UWP (PackageManager).
//
// Storage: отдельный SQLite `app-index.db` рядом с `ark.db` (per-instance slot).
// НЕ в ARK — app-индекс host-specific и regenerable, не должен синхронизироваться
// между устройствами и не должен загрязнять sync_version_vector.
// Frecency tracking остаётся в ARK (usage_event_obj) — join в-памяти.
//
// См. spec: `.agent/tasks/2026-05-22-app-launcher/spec.md`.

pub mod app;
pub mod exe_info;
pub mod icons;
pub mod platform;
pub mod ranking;
pub mod store;
pub mod watcher;

pub use app::{App, AppKind};

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum AppIndexError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("source '{0}' discover failed: {1}")]
    Discover(String, String),
    #[error("launch failed: {0}")]
    Launch(String),
    #[error("app not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppIndexError>;

/// Пауза (мс) между реальными cold extraction/write попытками, чтобы icon-storm
/// не бил DWM/GDI/Defender пачкой. Override через
/// `MUNDUS_APP_ICON_EXTRACT_SLEEP_MS`; default 50мс; `0` — без пауз.
fn icon_extract_sleep() -> u64 {
    std::env::var("MUNDUS_APP_ICON_EXTRACT_SLEEP_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(50)
}

struct IconExtractionQueue {
    cache_dir: PathBuf,
    sleep: std::time::Duration,
}

#[derive(Debug, Clone, Default)]
struct IconExtractionStats {
    extracted: u64,
    cached: u64,
    failed: u64,
    last_icon_ms: u64,
}

impl IconExtractionQueue {
    fn new(cache_dir: PathBuf, sleep_ms: u64) -> Self {
        Self {
            cache_dir,
            sleep: std::time::Duration::from_millis(sleep_ms),
        }
    }

    fn fill_missing_icons(&self, apps: &mut [App]) -> IconExtractionStats {
        let mut stats = IconExtractionStats::default();

        for app in apps {
            if app.icon_path.is_some() {
                continue;
            }

            let cached_path = icons::cached_icon_path(&self.cache_dir, &app.exec_path);
            let was_cached = cached_path.exists();
            let started = std::time::Instant::now();
            match icons::ensure_icon(&self.cache_dir, app) {
                Ok(path) => {
                    stats.last_icon_ms =
                        started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                    app.icon_path = Some(path);
                    if was_cached {
                        stats.cached += 1;
                    } else {
                        stats.extracted += 1;
                    }
                }
                Err(_) => {
                    stats.last_icon_ms =
                        started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                    stats.failed += 1;
                }
            }

            if !was_cached && !self.sleep.is_zero() {
                std::thread::sleep(self.sleep);
            }
        }

        stats
    }
}

/// Один источник приложений (Start Menu, UWP, etc.).
///
/// Один платформенный модуль может регистрировать несколько источников —
/// см. `platform::windows::sources()`.
pub trait AppSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn discover(&self) -> Result<Vec<App>>;
}

/// Memoized per-exe metadata for the usage report (KOS-287): version-info
/// display name + icon PNG. Extraction failures are cached too — re-reading a
/// permanently iconless exe on every refresh would just burn FFI calls.
#[derive(Debug, Clone, Default)]
pub struct ExeInfo {
    pub display_name: Option<String>,
    pub icon_path: Option<String>,
}

/// Реестр всех источников + кэш + SQLite store.
///
/// Singleton per backend process, shared через `Arc`.
pub struct AppIndex {
    sources: Arc<Vec<Box<dyn AppSource>>>,
    store: Arc<store::AppStore>,
    cache: Arc<RwLock<Vec<App>>>,
    icon_cache_dir: PathBuf,
    exe_info_cache: std::sync::Mutex<std::collections::HashMap<String, ExeInfo>>,
    last_rescan_ms: AtomicU64,
    icon_reads_count: AtomicU64,
    icon_bytes_read: AtomicU64,
    // Diagnostics: реально ли последний rescan шёл на background-priority потоке
    // (THREAD_MODE_BACKGROUND_BEGIN активирован). Подтверждает AC1/AC3.
    scan_background_mode: AtomicBool,
    discover_active: AtomicBool,
    icons_extracted: AtomicU64,
    icons_cached: AtomicU64,
    icons_failed: AtomicU64,
    last_icon_ms: AtomicU64,
    icon_sleep_ms: AtomicU64,
}

impl AppIndex {
    /// Создать новый AppIndex c default sources для текущей платформы.
    /// `data_dir` — где лежит app-index.db (per-instance, рядом с ark.db).
    /// `icon_cache_dir` — где лежат PNG иконки (per-instance localData).
    pub fn new(data_dir: &std::path::Path, icon_cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        std::fs::create_dir_all(&icon_cache_dir)?;

        let db_path = data_dir.join("app-index.db");
        let store = Arc::new(store::AppStore::open(&db_path)?);

        // Загружаем cached apps в memory сразу — hot path для search <10ms.
        let cached = store.list_all()?;
        let cache = Arc::new(RwLock::new(cached));

        let sources = platform::default_sources(icon_cache_dir.clone());

        Ok(Self {
            sources: Arc::new(sources),
            store,
            cache,
            icon_cache_dir,
            exe_info_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
            last_rescan_ms: AtomicU64::new(0),
            icon_reads_count: AtomicU64::new(0),
            icon_bytes_read: AtomicU64::new(0),
            scan_background_mode: AtomicBool::new(false),
            discover_active: AtomicBool::new(false),
            icons_extracted: AtomicU64::new(0),
            icons_cached: AtomicU64::new(0),
            icons_failed: AtomicU64::new(0),
            last_icon_ms: AtomicU64::new(0),
            icon_sleep_ms: AtomicU64::new(icon_extract_sleep()),
        })
    }

    /// Per-exe metadata for the usage report: version-info display name +
    /// icon PNG extracted on demand through the shared `icons::ensure_icon`
    /// cache (no second extractor). Blocking FFI — call from a blocking-pool
    /// thread; results are memoized for the process lifetime.
    pub fn exe_info(&self, exec_path: &str) -> ExeInfo {
        if let Some(hit) = self
            .exe_info_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_path)
        {
            return hit.clone();
        }
        let app = App {
            id: String::new(),
            name: String::new(),
            exec_path: exec_path.to_string(),
            icon_path: None,
            icon_source: None,
            kind: AppKind::Win32,
            source: "usage_tracker".to_string(),
            mtime: 0,
        };
        let info = ExeInfo {
            display_name: exe_info::exe_display_name(exec_path),
            icon_path: icons::ensure_icon(&self.icon_cache_dir, &app).ok(),
        };
        self.exe_info_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(exec_path.to_string(), info.clone());
        info
    }

    /// Force re-index — пройти все sources, обновить SQLite + cache.
    /// Возвращает diff stats для broadcast'а через command bus.
    pub async fn rescan(&self) -> Result<RescanStats> {
        let started = std::time::Instant::now();

        // Discover (WinRT UWP enum + Start Menu walk) и icon extraction (GDI →
        // PNG → запись файла) — синхронная блокирующая работа. Выносим на
        // blocking-pool поток с понижённым background приоритетом (CPU + I/O),
        // чтобы холодный icon-storm не насыщал диск и не душил систему. Никаких
        // `.await` внутри closure — guard снимается на том же потоке. Между
        // извлечениями иконок — throttle, чтобы не бить диск пачкой.
        let sources = self.sources.clone();
        let icon_cache_dir = self.icon_cache_dir.clone();
        let icon_sleep = icon_extract_sleep();
        self.discover_active.store(true, Ordering::SeqCst);
        self.icon_sleep_ms.store(icon_sleep, Ordering::SeqCst);
        let scan_result = tokio::task::spawn_blocking(move || {
            // Guard живёт внутри sync closure (без `.await`) — снимается на том же
            // потоке. is_active() фиксируем для diagnostics (подтверждение AC1/AC3).
            let bg = crate::priority::BackgroundThreadGuard::enter();
            let background_mode = bg.is_active();
            let mut all_apps: Vec<App> = Vec::new();
            let mut errors: Vec<String> = Vec::new();
            let mut failed_sources: Vec<&'static str> = Vec::new();

            for src in sources.iter() {
                match src.discover() {
                    Ok(apps) => {
                        tracing::info!(
                            target: "app_index",
                            source = src.name(),
                            count = apps.len(),
                            "discovered apps"
                        );
                        all_apps.extend(apps);
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "app_index",
                            source = src.name(),
                            error = %e,
                            "source discover failed"
                        );
                        errors.push(format!("{}: {e}", src.name()));
                        failed_sources.push(src.name());
                    }
                }
            }

            let icon_stats = IconExtractionQueue::new(icon_cache_dir, icon_sleep)
                .fill_missing_icons(&mut all_apps);

            (
                all_apps,
                errors,
                failed_sources,
                background_mode,
                icon_stats,
            )
        })
        .await;
        self.discover_active.store(false, Ordering::SeqCst);
        let (mut all_apps, errors, failed_sources, background_mode, icon_stats) = scan_result
            .map_err(|e| AppIndexError::Other(format!("rescan background join failed: {e}")))?;
        self.scan_background_mode
            .store(background_mode, Ordering::SeqCst);
        self.icons_extracted
            .store(icon_stats.extracted, Ordering::SeqCst);
        self.icons_cached.store(icon_stats.cached, Ordering::SeqCst);
        self.icons_failed.store(icon_stats.failed, Ordering::SeqCst);
        self.last_icon_ms
            .store(icon_stats.last_icon_ms, Ordering::SeqCst);

        // Diff против existing cache для stats.
        let existing = self.cache.read().await.clone();
        for app in &existing {
            if failed_sources.contains(&app.source.as_str())
                && !all_apps.iter().any(|candidate| candidate.id == app.id)
            {
                all_apps.push(app.clone());
            }
        }
        let stats = compute_diff(&existing, &all_apps);

        // Запись в SQLite + cache (atomic enough — SQLite транзакцией, cache swap'ом).
        let store = self.store.clone();
        let (all_apps, write_background_mode) = tokio::task::spawn_blocking(move || {
            let bg = crate::priority::BackgroundThreadGuard::enter();
            let background_mode = bg.is_active();
            store.replace_all(&all_apps)?;
            Ok::<_, AppIndexError>((all_apps, background_mode))
        })
        .await
        .map_err(|e| AppIndexError::Other(format!("rescan store join failed: {e}")))??;
        self.scan_background_mode
            .store(background_mode && write_background_mode, Ordering::SeqCst);

        *self.cache.write().await = all_apps;

        if !errors.is_empty() {
            tracing::warn!(target: "app_index", errors = ?errors, "rescan partial errors");
        }
        self.last_rescan_ms.store(
            started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            Ordering::SeqCst,
        );

        Ok(stats)
    }

    /// Поиск приложений по query. Возвращает top-N результатов c score.
    /// Frecency — отдельный аргумент (usage events из ARK), join в-памяти.
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        recent_usage: &ranking::UsageStats,
    ) -> Vec<ranking::ScoredApp> {
        let cache = self.cache.read().await;
        ranking::rank(&cache, query, limit, recent_usage)
    }

    /// Найти приложение по id (для launch).
    pub async fn find(&self, id: &str) -> Option<App> {
        self.cache.read().await.iter().find(|a| a.id == id).cloned()
    }

    /// Все приложения (с лимитом). Используется когда launcher показывает
    /// apps как часть общего списка без отдельного поиска.
    pub async fn all(&self, limit: usize) -> Vec<App> {
        self.cache
            .read()
            .await
            .iter()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Список источников (для launch dispatch).
    pub fn sources(&self) -> &[Box<dyn AppSource>] {
        self.sources.as_slice()
    }

    /// Запустить приложение через подходящий source.
    pub fn launch(&self, app: &App) -> Result<()> {
        platform::launch(app)
    }

    pub fn observe_icon_read(&self, bytes: usize) {
        self.icon_reads_count.fetch_add(1, Ordering::SeqCst);
        self.icon_bytes_read
            .fetch_add(bytes as u64, Ordering::SeqCst);
    }

    pub async fn diagnostics_snapshot(&self) -> AppIndexDiagnosticsSnapshot {
        AppIndexDiagnosticsSnapshot {
            apps_count: self.cache.read().await.len(),
            last_rescan_ms: self.last_rescan_ms.load(Ordering::SeqCst),
            icon_reads_count: self.icon_reads_count.load(Ordering::SeqCst),
            icon_bytes_read: self.icon_bytes_read.load(Ordering::SeqCst),
            scan_background_mode: self.scan_background_mode.load(Ordering::SeqCst),
            background_worker: AppIndexBackgroundWorkerDiagnostics {
                discover_active: self.discover_active.load(Ordering::SeqCst),
                icons_extracted: self.icons_extracted.load(Ordering::SeqCst),
                icons_cached: self.icons_cached.load(Ordering::SeqCst),
                icons_failed: self.icons_failed.load(Ordering::SeqCst),
                last_icon_ms: self.last_icon_ms.load(Ordering::SeqCst),
                sleep_ms: self.icon_sleep_ms.load(Ordering::SeqCst),
                background_mode: self.scan_background_mode.load(Ordering::SeqCst),
            },
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RescanStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub total: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppIndexDiagnosticsSnapshot {
    pub apps_count: usize,
    pub last_rescan_ms: u64,
    pub icon_reads_count: u64,
    pub icon_bytes_read: u64,
    /// Шёл ли последний rescan на background-priority потоке (CPU+I/O).
    pub scan_background_mode: bool,
    pub background_worker: AppIndexBackgroundWorkerDiagnostics,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppIndexBackgroundWorkerDiagnostics {
    pub discover_active: bool,
    pub icons_extracted: u64,
    pub icons_cached: u64,
    pub icons_failed: u64,
    pub last_icon_ms: u64,
    pub sleep_ms: u64,
    pub background_mode: bool,
}

fn compute_diff(old: &[App], new: &[App]) -> RescanStats {
    let old_ids: std::collections::HashSet<&str> = old.iter().map(|a| a.id.as_str()).collect();
    let new_ids: std::collections::HashSet<&str> = new.iter().map(|a| a.id.as_str()).collect();

    let added = new_ids.difference(&old_ids).count();
    let removed = old_ids.difference(&new_ids).count();

    // Updated = существовал и mtime поменялся.
    let old_map: std::collections::HashMap<&str, i64> =
        old.iter().map(|a| (a.id.as_str(), a.mtime)).collect();
    let updated = new
        .iter()
        .filter(|a| {
            old_map
                .get(a.id.as_str())
                .map(|old_mtime| *old_mtime != a.mtime)
                .unwrap_or(false)
        })
        .count();

    RescanStats {
        added,
        updated,
        removed,
        total: new.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    struct StaticSource {
        apps: Vec<App>,
    }

    impl AppSource for StaticSource {
        fn name(&self) -> &'static str {
            "static"
        }

        fn discover(&self) -> Result<Vec<App>> {
            Ok(self.apps.clone())
        }
    }

    #[tokio::test]
    async fn rescan_commits_store_then_swaps_cache() {
        let dir = tempdir().unwrap();
        let icon_dir = dir.path().join("icons");
        std::fs::create_dir_all(&icon_dir).unwrap();
        let app = App {
            id: "static-app".into(),
            name: "Static App".into(),
            exec_path: "C:\\static.exe".into(),
            icon_path: Some("C:\\icons\\static.png".into()),
            icon_source: None,
            kind: AppKind::Win32,
            source: "static".into(),
            mtime: 1,
        };
        let store = Arc::new(store::AppStore::open(&dir.path().join("app-index.db")).unwrap());
        let index = AppIndex {
            sources: Arc::new(vec![Box::new(StaticSource {
                apps: vec![app.clone()],
            })]),
            store: store.clone(),
            cache: Arc::new(RwLock::new(Vec::new())),
            icon_cache_dir: icon_dir,
            exe_info_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
            last_rescan_ms: AtomicU64::new(0),
            icon_reads_count: AtomicU64::new(0),
            icon_bytes_read: AtomicU64::new(0),
            scan_background_mode: AtomicBool::new(false),
            discover_active: AtomicBool::new(false),
            icons_extracted: AtomicU64::new(0),
            icons_cached: AtomicU64::new(0),
            icons_failed: AtomicU64::new(0),
            last_icon_ms: AtomicU64::new(0),
            icon_sleep_ms: AtomicU64::new(icon_extract_sleep()),
        };

        // Regression: 2026-06-09. SQLite replace_all must run inside rescan's
        // blocking commit path before the async-side cache swap publishes data.
        let stats = index.rescan().await.unwrap();

        assert_eq!(stats.added, 1);
        let cached = index.all(10).await;
        let persisted = store.list_all().unwrap();

        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].id, app.id);
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].id, app.id);
    }

    #[tokio::test]
    async fn diagnostics_exposes_app_index_background_worker() {
        let dir = tempdir().unwrap();
        let icon_dir = dir.path().join("icons");
        std::fs::create_dir_all(&icon_dir).unwrap();
        let store = Arc::new(store::AppStore::open(&dir.path().join("app-index.db")).unwrap());
        let index = AppIndex {
            sources: Arc::new(Vec::new()),
            store,
            cache: Arc::new(RwLock::new(Vec::new())),
            icon_cache_dir: icon_dir,
            exe_info_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
            last_rescan_ms: AtomicU64::new(0),
            icon_reads_count: AtomicU64::new(0),
            icon_bytes_read: AtomicU64::new(0),
            scan_background_mode: AtomicBool::new(true),
            discover_active: AtomicBool::new(false),
            icons_extracted: AtomicU64::new(2),
            icons_cached: AtomicU64::new(3),
            icons_failed: AtomicU64::new(1),
            last_icon_ms: AtomicU64::new(7),
            icon_sleep_ms: AtomicU64::new(50),
        };

        // Regression: 2026-06-09. Diagnostics must prove icon extraction is a
        // throttled background queue, not just expose scan_background_mode.
        let snapshot = index.diagnostics_snapshot().await;

        assert!(!snapshot.background_worker.discover_active);
        assert_eq!(snapshot.background_worker.icons_extracted, 2);
        assert_eq!(snapshot.background_worker.icons_cached, 3);
        assert_eq!(snapshot.background_worker.icons_failed, 1);
        assert_eq!(snapshot.background_worker.last_icon_ms, 7);
        assert_eq!(snapshot.background_worker.sleep_ms, 50);
        assert!(snapshot.background_worker.background_mode);
    }
}
