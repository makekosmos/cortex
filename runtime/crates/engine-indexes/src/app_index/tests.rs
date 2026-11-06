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
