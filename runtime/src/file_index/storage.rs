use super::*;

impl FileIndex {
    pub(super) fn root_paths(&self) -> Result<Vec<PathBuf>> {
        Ok(self
            .root_strings()?
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }

    pub(super) fn root_strings(&self) -> Result<Vec<String>> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        self.store.roots()
    }

    pub(super) fn index_enabled(&self) -> Result<bool> {
        Ok(self.enabled && self.store.enabled()?)
    }

    pub fn has_roots(&self) -> Result<bool> {
        Ok(!self.root_strings()?.is_empty())
    }

    pub(super) fn scan_options(&self) -> Result<ScanOptions> {
        Ok(ScanOptions {
            exclude_noisy_folders: self.store.exclude_noisy_folders()?,
            respect_gitignore: self.store.respect_gitignore()?,
            include_hidden: self.store.include_hidden()?,
            ntfs_accelerated: self.store.ntfs_accelerated()?,
            ignore_patterns: self.store.ignore_patterns()?,
        })
    }

    pub(super) fn restart_watcher(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let roots = self.root_paths()?;
        let next = watcher::start(&roots, self.store.clone());
        let mut watcher = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
        *watcher = next;
        Ok(())
    }

    pub(super) fn stop_watcher(&self) {
        let mut watcher = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
        *watcher = None;
    }
}
