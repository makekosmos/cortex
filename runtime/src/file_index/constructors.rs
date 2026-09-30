use super::*;

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

    pub(super) fn with_roots_internal(
        data_dir: &Path,
        roots: Vec<PathBuf>,
        enabled: bool,
    ) -> Result<Self> {
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
            background_tasks: StdMutex::new(Vec::new()),
            ntfs_last_state: Arc::new(StdMutex::new(NtfsState::default())),
            last_scan: StdMutex::new(None),
            enabled,
        })
    }

    pub fn bind_self(self: &Arc<Self>) {
        let mut self_ref = self.self_ref.lock().unwrap_or_else(|e| e.into_inner());
        *self_ref = Arc::downgrade(self);
    }
}
