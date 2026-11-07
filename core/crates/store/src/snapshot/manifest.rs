use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{backups_dir, open_no_follow, schema_fingerprint, sync_file};

const MANIFEST_SUFFIX: &str = ".manifest.json";
const MANIFEST_VERSION: u32 = 1;
const MANIFEST_MAX_BYTES: u64 = 16 * 1024;
pub(super) const BACKUP_PREFIX: &str = "ark.db.backup-v1-";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotManifest {
    version: u32,
    id: String,
    size_bytes: u64,
    sha256: String,
    schema_sha256: String,
}

/// Publish a verified snapshot. The manifest is the visibility marker: readers
/// ignore a database without it and a manifest without its database.
pub fn backup_to_file_chunked_verified(
    src_db_path: &str,
    dest_path: &str,
    pages_per_step: i32,
    pause: Duration,
) -> Result<(), String> {
    let dest = Path::new(dest_path);
    let parent = dest
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "backup destination must have a parent directory".to_string())?;
    let id = dest
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "backup destination name is not UTF-8".to_string())?;
    let manifest = manifest_path(parent, id);
    if dest.exists() || manifest.exists() {
        return Err(format!("backup destination already exists: {dest_path}"));
    }

    let nonce = format!("{:016x}", rand::random::<u64>());
    let staged_db = parent.join(format!(".ark-backup-stage-{nonce}.db"));
    let staged_manifest = parent.join(format!(".ark-backup-manifest-{nonce}.tmp"));
    let mut manifest_published = false;
    let result = (|| -> Result<(), String> {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_db)
            .map_err(|error| format!("backup staging create failed: {error}"))?;
        crate::open::backup_to_file_chunked_unpublished(
            src_db_path,
            staged_db
                .to_str()
                .ok_or_else(|| "backup staging path is not UTF-8".to_string())?,
            pages_per_step,
            pause,
        )?;

        let staged = Connection::open_with_flags(&staged_db, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| format!("backup verification open failed: {error}"))?;
        crate::open::check_integrity(&staged)?;
        let schema_sha256 = digest_bytes(
            &serde_json::to_vec(&schema_fingerprint(&staged)?)
                .map_err(|error| format!("backup schema fingerprint encode failed: {error}"))?,
        );
        drop(staged);

        let size_bytes = fs::metadata(&staged_db)
            .map_err(|error| format!("backup staging metadata failed: {error}"))?
            .len();
        let sha256 = digest_file(&staged_db)?;
        sync_file(&staged_db).map_err(|error| format!("backup staging sync failed: {error}"))?;
        let bytes = serde_json::to_vec(&SnapshotManifest {
            version: MANIFEST_VERSION,
            id: id.to_string(),
            size_bytes,
            sha256,
            schema_sha256,
        })
        .map_err(|error| format!("backup manifest encode failed: {error}"))?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_manifest)
            .map_err(|error| format!("backup manifest staging create failed: {error}"))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("backup manifest write failed: {error}"))?;

        // hard_link is an atomic no-clobber publication on supported local filesystems.
        // The manifest goes first: a crash at either boundary leaves an ignored orphan.
        fs::hard_link(&staged_manifest, &manifest)
            .map_err(|error| format!("backup manifest publish failed: {error}"))?;
        manifest_published = true;
        fs::hard_link(&staged_db, dest)
            .map_err(|error| format!("backup database publish failed: {error}"))?;
        sync_directory(parent).map_err(|error| format!("backup directory sync failed: {error}"))?;
        Ok(())
    })();
    remove_snapshot_file_and_sidecars(&staged_db);
    let _ = fs::remove_file(&staged_manifest);
    if result.is_err() && manifest_published {
        let _ = fs::remove_file(&manifest);
    }
    result
}

fn remove_snapshot_file_and_sidecars(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-shm", "-wal", "-journal"] {
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }
}

fn manifest_path(parent: &Path, id: &str) -> PathBuf {
    parent.join(format!("{id}{MANIFEST_SUFFIX}"))
}

pub(super) fn is_manifest_file(id: &str) -> bool {
    id.ends_with(MANIFEST_SUFFIX)
}

pub(super) fn is_uncommitted_backup(dir: &Path, id: &str) -> bool {
    id.starts_with(BACKUP_PREFIX) && !manifest_path(dir, id).exists()
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest_file(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|error| format!("backup hash open failed: {error}"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("backup hash read failed: {error}"))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    // Windows std currently exposes no portable directory FlushFileBuffers path.
    // Snapshot file contents are synced, but directory names may be lost on power failure.
    Ok(())
}

pub(super) fn verify_snapshot_manifest(
    db_path: &str,
    id: &str,
    staged: &Path,
) -> Result<(), String> {
    let parent = backups_dir(db_path);
    let path = manifest_path(&parent, id);
    let bytes = match open_no_follow(&path) {
        Ok(mut file) => {
            let length = file
                .metadata()
                .map_err(|error| format!("snapshot manifest metadata failed: {error}"))?
                .len();
            if length > MANIFEST_MAX_BYTES {
                return Err("snapshot manifest exceeds size limit".to_string());
            }
            let mut bytes = Vec::with_capacity(length as usize);
            file.read_to_end(&mut bytes)
                .map_err(|error| format!("snapshot manifest read failed: {error}"))?;
            bytes
        }
        Err(_error)
            if matches!(
                fs::symlink_metadata(&path),
                Err(ref metadata_error) if metadata_error.kind() == io::ErrorKind::NotFound
            ) && !id.starts_with(BACKUP_PREFIX) =>
        {
            return Ok(());
        }
        Err(error) => return Err(format!("snapshot manifest unavailable: {error}")),
    };
    let manifest: SnapshotManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("snapshot manifest is invalid: {error}"))?;
    if manifest.version != MANIFEST_VERSION || manifest.id != id {
        return Err("snapshot manifest identity or version mismatch".to_string());
    }
    let size_bytes = fs::metadata(staged)
        .map_err(|error| format!("staged snapshot metadata failed: {error}"))?
        .len();
    if manifest.size_bytes != size_bytes || manifest.sha256 != digest_file(staged)? {
        return Err("snapshot manifest checksum mismatch".to_string());
    }
    let staged_conn = Connection::open_with_flags(staged, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("staged snapshot open failed: {error}"))?;
    crate::open::check_integrity(&staged_conn)?;
    let schema = serde_json::to_vec(&schema_fingerprint(&staged_conn)?)
        .map_err(|error| format!("snapshot schema fingerprint encode failed: {error}"))?;
    if manifest.schema_sha256 != digest_bytes(&schema) {
        return Err("snapshot manifest schema checksum mismatch".to_string());
    }
    Ok(())
}
