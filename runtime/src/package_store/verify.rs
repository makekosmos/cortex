//! Verified reads over the immutable blobs: entrypoint/asset lookup and the
//! deny-write open whose recorded identity replaces the per-call re-hash
//! (KOS-290).

use super::identity;
use super::paths::safe_asset_path;
use super::*;

impl PackageStore {
    pub fn immutable_entrypoint(&self, package: &InstalledPackage) -> Result<PathBuf, StoreError> {
        let entrypoint = package
            .manifest
            .worker_entrypoint()
            .ok_or(StoreError::WorkerRequired)?;
        let canonical = self.immutable_asset_path(package, entrypoint)?;
        if !canonical
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
        {
            return Err(StoreError::Archive("invalid worker entrypoint".into()));
        }
        Ok(canonical)
    }

    /// Return a verified, unpacked static asset path for an installed package.
    pub fn immutable_asset_path(
        &self,
        package: &InstalledPackage,
        entry: &str,
    ) -> Result<PathBuf, StoreError> {
        if !safe_asset_path(entry) {
            return Err(StoreError::Archive("invalid asset path".into()));
        }
        let root = self
            .root
            .join("unpacked")
            .join(&package.id)
            .join(&package.version)
            .join(&package.hash);
        let path = root.join(entry);
        let canonical_root = fs::canonicalize(&root).map_err(StoreError::Io)?;
        let canonical = fs::canonicalize(&path).map_err(StoreError::Io)?;
        if !canonical.starts_with(&canonical_root) || !canonical.is_file() {
            return Err(StoreError::Archive("invalid asset path".into()));
        }
        let blob = self
            .root
            .join("blobs")
            .join(format!("{}.kspkg", package.hash));
        let mut archive = ZipArchive::new(open_verified_blob(&blob, &package.hash)?)?;
        let archived_hash = hash_reader(archive.by_name(entry)?)?;
        if archived_hash != hash_reader(fs::File::open(&canonical)?)? {
            return Err(StoreError::HashMismatch);
        }
        Ok(canonical)
    }

    /// Read one static asset directly from the verified immutable archive.
    /// Unpacked package files are deliberately never consulted here.
    pub fn read_blob_entry(
        &self,
        package: &InstalledPackage,
        entry: &str,
    ) -> Result<Vec<u8>, StoreError> {
        if !safe_asset_path(entry) {
            return Err(StoreError::Archive("invalid asset path".into()));
        }
        let blob = self
            .root
            .join("blobs")
            .join(format!("{}.kspkg", package.hash));
        let blob_file = open_verified_blob(&blob, &package.hash)?;
        let mut archive = ZipArchive::new(blob_file)?;
        let file = archive
            .by_name(entry)
            .map_err(|_| StoreError::Archive("asset not found".into()))?;
        if file.is_dir() {
            return Err(StoreError::Archive("asset is a directory".into()));
        }
        if file.size() > MAX_ASSET_BYTES {
            return Err(StoreError::Archive("asset too large".into()));
        }
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.take(MAX_ASSET_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_ASSET_BYTES {
            return Err(StoreError::Archive("asset too large".into()));
        }
        Ok(bytes)
    }

    /// Re-check an unpacked entrypoint against its immutable archive bytes.
    /// The launcher calls this immediately before spawning a worker.
    ///
    /// Threat model (the blob lives at `<store>/blobs/<hash>.kspkg` under the
    /// per-user data dir, writable only by the owning user). A same-user
    /// process is out of scope entirely — it can rewrite the blob, the
    /// identity record, `state.json`, or the Engine binaries (never verified
    /// at launch) — before and after KOS-290 alike. The rows below compare
    /// with the old always-hash check for the attackers that remain:
    /// another user, or plain corruption.
    /// - Tampering with the stored blob after install: another user has no
    ///   write access; accidental corruption moves the size or timestamps
    ///   → identity miss → full hash. Same as before — except in-place
    ///   corruption that changes *no* metadata, which the per-launch hash
    ///   caught and the fast path does not: weaker, accepted (the bytes are
    ///   still hashed at install, at unpack, and on any identity mismatch).
    /// - TOCTOU between verification and the worker opening the entrypoint:
    ///   the unpacked `.exe` itself is hashed on every launch (one archive
    ///   entry, cheap), and the blob is held with write sharing denied for
    ///   the whole read so it cannot be modified or swapped under the
    ///   handle. Same as before — the mechanism is unchanged.
    /// - A swapped file at the same path (delete+recreate, rename-over,
    ///   restored backup): a new file ID → full re-verify, re-binding the
    ///   record only when the bytes still are the verified blob. Same as
    ///   before.
    /// - A hard link or junction redirecting the path: a link to the same
    ///   file keeps its identity (still the verified bytes); a different
    ///   file behind the path fails the identity check and re-verifies.
    ///   Same as before.
    ///
    /// Separately, a *reference* swap — pointing `state.json` or the
    /// `unpacked/<hash>` directory at differently-hashed content — is a
    /// store-integrity question, not a content check; for catalog packages
    /// in release builds `prepare_worker_launch` additionally pins
    /// `installed.hash` to the signed catalog entry.
    pub(crate) fn verify_immutable_entrypoint_path(path: &Path) -> Result<(), StoreError> {
        let canonical = fs::canonicalize(path)?;
        let Some(hash_dir) = canonical.ancestors().find(|candidate| {
            let Some(name) = candidate.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            is_hash(name)
                && candidate
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::parent)
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "unpacked")
        }) else {
            return Err(StoreError::Archive("invalid worker entrypoint".into()));
        };
        let hash = hash_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| StoreError::Archive("invalid worker entrypoint".into()))?;
        let relative = canonical
            .strip_prefix(hash_dir)
            .map_err(|_| StoreError::Archive("invalid worker entrypoint".into()))?;
        let entrypoint = relative.to_string_lossy().replace('\\', "/");
        if entrypoint.is_empty() || entrypoint.contains("..") {
            return Err(StoreError::Archive("invalid worker entrypoint".into()));
        }
        let store_root = hash_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or_else(|| StoreError::Archive("invalid worker entrypoint".into()))?;
        let blob = store_root.join("blobs").join(format!("{hash}.kspkg"));
        let blob_file = open_verified_blob(&blob, hash)?;
        let mut archive = ZipArchive::new(blob_file)?;
        let archived_hash = hash_reader(archive.by_name(&entrypoint)?)?;
        let unpacked_hash = hash_reader(fs::File::open(canonical)?)?;
        if archived_hash != unpacked_hash {
            return Err(StoreError::HashMismatch);
        }
        Ok(())
    }
}

/// Open an immutable blob deny-write and return a handle positioned at 0
/// whose bytes are known to still be the verified `expected_hash` contents.
///
/// A blob with a matching recorded identity (see `identity`) skips the
/// SHA-256 entirely; any doubt runs the full hash and repairs the record.
fn open_verified_blob(blob: &Path, expected_hash: &str) -> Result<fs::File, StoreError> {
    let mut file = open_immutable_read(blob)?;
    let current = identity::of(&file).ok();
    let recorded = identity::load(blob);
    if let (Some(recorded), Some(current)) = (recorded, current.as_ref()) {
        if recorded == *current {
            return Ok(file);
        }
    }
    #[cfg(test)]
    FULL_BLOB_HASHES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(blob.to_path_buf());
    if !eq_hash(&hash_reader(&mut file)?, expected_hash) {
        return Err(StoreError::HashMismatch);
    }
    // The bytes are verified, so a failed record write stays non-fatal — but
    // without the record every launch silently re-pays the full hash, which
    // is the exact regression this fast path exists to remove; log it.
    if current.is_some() {
        if let Err(error) = identity::store(blob, &file) {
            tracing::warn!(
                blob = %blob.display(),
                %error,
                "package blob identity record not written; launches will re-hash"
            );
        }
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(file)
}

/// Test seam: which blob paths had to take the full-hash path, so tests can
/// assert an unchanged blob was *not* re-hashed without relying on timing.
#[cfg(test)]
static FULL_BLOB_HASHES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

#[cfg(test)]
pub(super) fn full_blob_hash_count(blob: &Path) -> usize {
    // The launch path only sees the canonicalized store root, so recorded
    // entries are verbatim (`\\?\`-prefixed) paths — compare canonical forms.
    let canonical = fs::canonicalize(blob).unwrap_or_else(|_| blob.to_path_buf());
    FULL_BLOB_HASHES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|path| path.as_path() == blob || **path == canonical)
        .count()
}

pub(super) fn open_immutable_read(path: &Path) -> io::Result<fs::File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        return retry_io(|| fs::OpenOptions::new().read(true).share_mode(1).open(path));
    }
    #[cfg(not(windows))]
    {
        retry_io(|| fs::File::open(path))
    }
}
