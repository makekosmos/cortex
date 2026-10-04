//! Archive install: hash verification, staged extraction and the atomic
//! blob/unpacked placement.

use super::verify::open_immutable_read;
use super::*;

static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

impl PackageStore {
    pub fn install(
        &self,
        archive: impl AsRef<Path>,
        expected_size: u64,
        expected_hash: &str,
        expected_manifest: &PackageManifest,
        catalog_sequence: u64,
    ) -> Result<InstalledPackage, StoreError> {
        let _ = (
            archive,
            expected_size,
            expected_hash,
            expected_manifest,
            catalog_sequence,
        );
        Err(StoreError::Manifest(ManifestError::InvalidField(
            "schema_version",
        )))
    }

    pub fn install_versioned(
        &self,
        archive: impl AsRef<Path>,
        expected_size: u64,
        expected_hash: &str,
        expected_manifest: &VersionedManifest,
        catalog_sequence: u64,
    ) -> Result<InstalledPackage, StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(expected_manifest, VersionedManifest::V1(_)) {
            return Err(StoreError::Manifest(ManifestError::InvalidField(
                "schema_version",
            )));
        }
        expected_manifest.validate()?;
        let archive = archive.as_ref();
        let metadata = retry_io(|| fs::metadata(archive))?;
        if metadata.len() != expected_size || metadata.len() > MAX_ARCHIVE {
            return Err(if metadata.len() != expected_size {
                StoreError::SizeMismatch
            } else {
                StoreError::Archive("archive too large".into())
            });
        }
        let bytes = retry_io(|| fs::read(archive))?;
        let hash = hex_hash(&bytes);
        if !eq_hash(&hash, expected_hash) {
            return Err(StoreError::HashMismatch);
        }
        let file = retry_io(|| fs::File::open(archive))?;
        let mut zip = ZipArchive::new(file)?;
        if zip.len() > MAX_ENTRIES {
            return Err(StoreError::Archive("too many entries".into()));
        }
        let staging = self.root.join(format!(
            ".staging-{hash}-{}",
            STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        if staging.exists() {
            retry_io(|| fs::remove_dir_all(&staging))?;
        }
        retry_io(|| fs::create_dir_all(&staging))?;
        // Extracted trees nest deeply (hash + archive paths); canonicalize so
        // Windows gets extended-length paths instead of MAX_PATH denials.
        let staging = fs::canonicalize(&staging)?;
        let result = self.extract_verify(&mut zip, &staging, expected_manifest, &hash);
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        let blob = self.root.join("blobs").join(format!("{hash}.kspkg"));
        if !blob.exists() {
            let blob_tmp = self
                .root
                .join("blobs")
                .join(format!(".{hash}.{}.tmp", std::process::id()));
            retry_io(|| fs::copy(archive, &blob_tmp))?;
            retry_io(|| fs::rename(&blob_tmp, &blob))?;
        }
        // Record the verified blob's identity so later opens can skip the
        // re-hash (KOS-290). Failure degrades to the always-hash path, never
        // to a weaker check, so it must not fail the install — but a store
        // that can never write records would silently re-pay the 2 s hash
        // on every launch, so it is logged.
        match open_immutable_read(&blob) {
            Ok(file) => {
                if let Err(error) = identity::store(&blob, &file) {
                    tracing::warn!(
                        blob = %blob.display(),
                        %error,
                        "package blob identity record not written; launches will re-hash"
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    blob = %blob.display(),
                    %error,
                    "installed package blob unreadable for identity record; launches will re-hash"
                );
            }
        }
        let unpacked = fs::canonicalize(self.root.join("unpacked"))?
            .join(expected_manifest.id())
            .join(expected_manifest.version())
            .join(&hash);
        if let Some(parent) = unpacked.parent() {
            retry_io(|| fs::create_dir_all(parent))?;
        }
        if !unpacked.exists() {
            // Freshly extracted trees can be held open by indexer/AV scans on
            // Windows for several seconds; a directory move fails with
            // AccessDenied until the transient handle closes. Retry with
            // backoff; the operation is rare enough that a few seconds is fine.
            let mut attempt = 0u32;
            loop {
                match fs::rename(&staging, &unpacked) {
                    Ok(()) => break,
                    Err(error) => {
                        attempt += 1;
                        let retries_left =
                            attempt < 40 && matches!(error.kind(), io::ErrorKind::PermissionDenied);
                        if !retries_left {
                            return Err(error.into());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                }
            }
        } else {
            retry_io(|| fs::remove_dir_all(&staging))?;
        }
        let package = InstalledPackage {
            id: expected_manifest.id().to_owned(),
            version: expected_manifest.version().to_owned(),
            hash,
            manifest: expected_manifest.clone(),
            enabled: false,
            revoked: false,
            installed_at: now(),
            catalog_sequence,
        };
        let mut state = self.read_state()?;
        state
            .packages
            .retain(|p| p.id != package.id || p.version != package.version);
        state.packages.push(package.clone());
        self.write_state(&state)?;
        Ok(package)
    }

    fn extract_verify(
        &self,
        zip: &mut ZipArchive<fs::File>,
        staging: &Path,
        expected: &VersionedManifest,
        _hash: &str,
    ) -> Result<(), StoreError> {
        let mut total = 0u64;
        let mut names = std::collections::HashSet::new();
        let mut found_manifest = None;
        let mut found_entry = false;
        let mut missing_workers = match expected {
            VersionedManifest::V2(manifest) => manifest
                .declared_worker_entrypoints()
                .into_iter()
                .map(str::to_owned)
                .collect::<std::collections::HashSet<_>>(),
            VersionedManifest::V1(_) => std::collections::HashSet::new(),
        };
        let mut found_icon = expected.icon().is_none();
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let name = entry
                .enclosed_name()
                .ok_or_else(|| StoreError::Archive("unsafe path".into()))?
                .to_path_buf();
            let normalized =
                normalize_path(&name).ok_or_else(|| StoreError::Archive("unsafe path".into()))?;
            if !names.insert(normalized.to_ascii_lowercase()) {
                return Err(StoreError::Archive("duplicate path".into()));
            }
            let file_type = entry.unix_mode().map(|mode| mode & 0o170000);
            if file_type == Some(0o120000) {
                return Err(StoreError::Archive("symlink rejected".into()));
            }
            if entry.is_dir() {
                if let Some(file_type) = file_type {
                    if file_type != 0 && file_type != 0o040000 {
                        return Err(StoreError::Archive("unsupported file type".into()));
                    }
                }
                retry_io(|| fs::create_dir_all(staging.join(&name)))?;
                continue;
            }
            if let Some(file_type) = file_type {
                if file_type != 0 && file_type != 0o100000 {
                    return Err(StoreError::Archive("unsupported file type".into()));
                }
            }
            total = total.saturating_add(entry.size());
            if total > MAX_EXPANDED {
                return Err(StoreError::Archive("expanded archive too large".into()));
            }
            let out = staging.join(&name);
            if let Some(parent) = out.parent() {
                retry_io(|| fs::create_dir_all(parent))?;
            }
            let mut file = retry_io(|| fs::File::create(&out))?;
            let declared_size = entry.size();
            let copied = io::copy(
                &mut (&mut entry).take(declared_size.saturating_add(1)),
                &mut file,
            )?;
            if copied != declared_size {
                return Err(StoreError::Archive("entry size mismatch".into()));
            }
            if normalized.eq_ignore_ascii_case("manifest.json") {
                found_manifest = Some(retry_io(|| fs::read(&out))?);
            }
            if normalized == expected.entrypoint() {
                found_entry = true;
            }
            missing_workers.remove(&normalized);
            if expected.icon().is_some_and(|icon| normalized == icon) {
                found_icon = true;
            }
        }
        let raw =
            found_manifest.ok_or_else(|| StoreError::Archive("manifest.json missing".into()))?;
        let actual = PackageManifest::parse(
            std::str::from_utf8(&raw)
                .map_err(|_| StoreError::Archive("manifest is not UTF-8".into()))?,
        )
        .map_err(StoreError::Manifest)?;
        if &actual != expected {
            return Err(StoreError::ManifestMismatch);
        }
        if !found_entry {
            return Err(StoreError::Archive("entrypoint missing".into()));
        }
        if !missing_workers.is_empty() {
            return Err(StoreError::Archive("worker entrypoint missing".into()));
        }
        if !found_icon {
            return Err(StoreError::Archive("icon missing".into()));
        }
        // ZIP modes are untrusted (and our deterministic archives omit them).
        // Only validated, declared workers get owner execute permission.
        #[cfg(unix)]
        if let VersionedManifest::V2(manifest) = expected {
            use std::os::unix::fs::PermissionsExt;
            for entrypoint in manifest.declared_worker_entrypoints() {
                fs::set_permissions(staging.join(entrypoint), fs::Permissions::from_mode(0o700))?;
            }
        }
        Ok(())
    }
}
