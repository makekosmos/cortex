impl PackageService {
    pub fn build_engine_snapshot(
        &self,
        package_id: &str,
        source: &str,
    ) -> Result<Vec<PackageSnapshotFile>, PackageError> {
        if package_id.is_empty()
            || !package_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            || source != "installed" && source != "auto"
        {
            return Err(PackageError::Invalid);
        }
        let package = self
            .store
            .list()?
            .into_iter()
            .find(|item| item.id == package_id && !item.revoked && item.enabled)
            .ok_or(PackageError::Invalid)?;
        let root = self
            .root
            .join("unpacked")
            .join(&package.id)
            .join(&package.version)
            .join(&package.hash);
        let config =
            BrokerConfig::new(std::iter::empty::<&str>(), vec![self.root.join("unpacked")])
                .map_err(|_| PackageError::Invalid)?;
        read_snapshot_tree(&config, &root).map_err(|_| PackageError::Invalid)
    }
    pub fn engine_snapshot_identity(
        &self,
        package_id: &str,
        _source: &str,
    ) -> Result<(String, u64, u64), PackageError> {
        let package = self
            .store
            .list()?
            .into_iter()
            .find(|item| item.id == package_id && !item.revoked && item.enabled)
            .ok_or(PackageError::Invalid)?;
        let root = fs::canonicalize(
            self.root
                .join("unpacked")
                .join(&package.id)
                .join(&package.version)
                .join(&package.hash),
        )
        .map_err(|_| PackageError::Invalid)?;
        let metadata = fs::metadata(&root).map_err(|_| PackageError::Invalid)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            return Ok((
                root.to_string_lossy().into_owned(),
                metadata.dev(),
                metadata.ino(),
            ));
        }
        #[cfg(not(unix))]
        {
            let _ = (_source, metadata);
            Err(PackageError::Invalid)
        }
    }
    pub fn build_engine_grant_snapshot(
        &self,
        extension_id: &str,
        root: &Path,
        identity_dev: u64,
        identity_ino: u64,
        exact_file: bool,
    ) -> Result<Vec<PackageSnapshotFile>, PackageError> {
        if extension_id.is_empty() || !root.is_absolute() {
            return Err(PackageError::Invalid);
        }
        let configured_parent = self.root.parent().ok_or(PackageError::Invalid)?;
        let canonical = fs::canonicalize(root).map_err(|_| PackageError::Invalid)?;
        let configured_parent =
            fs::canonicalize(configured_parent).map_err(|_| PackageError::Invalid)?;
        if !(canonical == configured_parent || canonical.starts_with(&configured_parent)) {
            return Err(PackageError::Invalid);
        }
        let metadata = fs::metadata(&canonical).map_err(|_| PackageError::Invalid)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != identity_dev || metadata.ino() != identity_ino {
                return Err(PackageError::Invalid);
            }
        }
        #[cfg(not(unix))]
        let _ = (identity_dev, identity_ino, metadata);
        let snapshot_root = if exact_file {
            canonical.parent().ok_or(PackageError::Invalid)?
        } else {
            &canonical
        };
        let config = BrokerConfig::new(std::iter::empty::<&str>(), vec![configured_parent])
            .map_err(|_| PackageError::Invalid)?;
        let mut files =
            read_snapshot_tree(&config, snapshot_root).map_err(|_| PackageError::Invalid)?;
        if exact_file {
            let filename = canonical
                .file_name()
                .ok_or(PackageError::Invalid)?
                .to_string_lossy();
            files.retain(|file| file.path == filename);
        }
        if files.is_empty() {
            return Err(PackageError::Invalid);
        }
        Ok(files)
    }
}
