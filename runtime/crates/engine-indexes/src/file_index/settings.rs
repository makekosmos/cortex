use super::*;

impl FileIndex {
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
        let mut should_rescan = patch.enabled.is_none();
        if let Some(value) = patch.exclude_noisy_folders {
            self.store.set_exclude_noisy_folders(value)?;
        }
        if let Some(value) = patch.enabled {
            self.store.set_enabled(value)?;
            if !value {
                should_rescan = false;
                self.clear_progress_after_disable();
                self.stop_watcher();
            } else {
                self.restart_watcher()?;
                let snap = self.store.stats_snapshot()?;
                should_rescan = snap.total == 0 && !snap.roots.is_empty();
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
}
