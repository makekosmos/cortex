use super::*;

impl FileIndex {
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
    pub(super) fn current_stats(&self) -> Result<ScanStats> {
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
    pub(super) fn progress_snapshot(&self) -> ScanProgressSnapshot {
        self.scan_progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub(super) fn set_progress(&self, snapshot: ScanProgressSnapshot) {
        let mut progress = self.scan_progress.lock().unwrap_or_else(|e| e.into_inner());
        *progress = snapshot;
    }

    pub(super) fn clear_progress_after_disable(&self) {
        self.scan_in_progress.store(false, Ordering::SeqCst);
        self.set_progress(ScanProgressSnapshot {
            phase: "disabled".to_string(),
            message: "Поиск файлов выключен".to_string(),
            ..Default::default()
        });
    }

    pub(super) fn remember_last_scan(&self, snapshot: LastScanSnapshot) {
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

    pub(super) fn observe_search(&self, query: &str) {
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
}
