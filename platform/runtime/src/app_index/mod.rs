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
pub mod icons;
pub mod platform;
pub mod ranking;
pub mod store;
pub mod watcher;

pub use app::{App, AppKind};

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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

/// Один источник приложений (Start Menu, UWP, etc.).
///
/// Один платформенный модуль может регистрировать несколько источников —
/// см. `platform::windows::sources()`.
pub trait AppSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn discover(&self) -> Result<Vec<App>>;
}

/// Реестр всех источников + кэш + SQLite store.
///
/// Singleton per backend process, shared через `Arc`.
pub struct AppIndex {
    sources: Vec<Box<dyn AppSource>>,
    store: store::AppStore,
    cache: Arc<RwLock<Vec<App>>>,
    icon_cache_dir: PathBuf,
    last_rescan_ms: AtomicU64,
    icon_reads_count: AtomicU64,
    icon_bytes_read: AtomicU64,
}

impl AppIndex {
    /// Создать новый AppIndex c default sources для текущей платформы.
    /// `data_dir` — где лежит app-index.db (per-instance, рядом с ark.db).
    /// `icon_cache_dir` — где лежат PNG иконки (per-instance localData).
    pub fn new(data_dir: &std::path::Path, icon_cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        std::fs::create_dir_all(&icon_cache_dir)?;

        let db_path = data_dir.join("app-index.db");
        let store = store::AppStore::open(&db_path)?;

        // Загружаем cached apps в memory сразу — hot path для search <10ms.
        let cached = store.list_all()?;
        let cache = Arc::new(RwLock::new(cached));

        let sources = platform::default_sources(icon_cache_dir.clone());

        Ok(Self {
            sources,
            store,
            cache,
            icon_cache_dir,
            last_rescan_ms: AtomicU64::new(0),
            icon_reads_count: AtomicU64::new(0),
            icon_bytes_read: AtomicU64::new(0),
        })
    }

    /// Force re-index — пройти все sources, обновить SQLite + cache.
    /// Возвращает diff stats для broadcast'а через command bus.
    pub async fn rescan(&self) -> Result<RescanStats> {
        let started = std::time::Instant::now();
        let mut all_apps: Vec<App> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for src in &self.sources {
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
                }
            }
        }

        // Извлечь / переиспользовать иконки.
        for app in &mut all_apps {
            if app.icon_path.is_none() {
                if let Ok(path) = icons::ensure_icon(&self.icon_cache_dir, app) {
                    app.icon_path = Some(path);
                }
            }
        }

        // Diff против existing cache для stats.
        let existing = self.cache.read().await.clone();
        let stats = compute_diff(&existing, &all_apps);

        // Запись в SQLite + cache (atomic enough — SQLite транзакцией, cache swap'ом).
        self.store.replace_all(&all_apps)?;
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
        &self.sources
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
