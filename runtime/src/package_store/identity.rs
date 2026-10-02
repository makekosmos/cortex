//! Persistent identity binding for verified package blobs (KOS-290).
//!
//! Every package read used to re-hash the whole `.kspkg` blob (~2 s for a
//! 26 MB archive at each worker launch). The blob is already hashed at
//! install; what a later open needs is proof that the file on disk is still
//! *the same file* that was verified. A successful full hash therefore
//! records a `BlobIdentity` — volume serial, file ID, size, last-write and
//! metadata-change time — captured from an open handle. Later opens re-read
//! the identity from the live handle (a syscall, not a pass over the bytes)
//! and compare, while the deny-write share mode of `open_immutable_read`
//! keeps the file from being modified or replaced underneath.
//!
//! Why each field:
//! - `volume_serial` + `file_id`: a swapped file (delete + recreate,
//!   rename-over, restored backup) gets a new file ID even at the same path;
//! - `size` + `modified`: any write through the data stream moves them;
//! - `changed`: NTFS change time / unix ctime also move on *metadata*
//!   changes and — unlike mtime — cannot be rolled back with
//!   `SetFileTime`/`utimens`, so an in-place rewrite cannot hide behind
//!   restored timestamps. A filesystem that reports no change time is
//!   refused and always takes the full-hash path.
//!
//! The record lives next to the blob (`<hash>.identity.json`) with the same
//! owner-only permissions as `state.json`: forging it requires write access
//! to the per-user store, and anyone with that can already rewrite the store
//! wholesale. Anything doubtful — missing record (installs predating this
//! scheme), an unreadable or unparsable record, an identity query the
//! filesystem refuses, or a plain mismatch — falls back to the full SHA-256,
//! and the record is rewritten only when the bytes check out. The guarantee
//! never drops below the old always-hash behaviour.

use crate::lock_file::{read_owner_only_json, write_owner_only_json};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// `<hash>.kspkg` → `<hash>.identity.json`, kept in `blobs/` so the sweep of
/// interrupted writes covers its temp files too.
pub(crate) fn record_path(blob: &Path) -> PathBuf {
    blob.with_extension("identity.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlobIdentity {
    volume_serial: u64,
    file_id: String,
    size: u64,
    modified: i128,
    changed: i128,
}

/// Identity of the file behind an *open* handle — the values are read from
/// the handle itself, so a path swap cannot desynchronize them.
pub(crate) fn of(file: &fs::File) -> io::Result<BlobIdentity> {
    platform_identity(file)
}

/// The recorded identity for `blob`, or `None` when the record is absent or
/// unreadable — both take the same full-hash path, so a corrupt record is
/// indistinguishable from a missing one and self-repairs.
pub(crate) fn load(blob: &Path) -> Option<BlobIdentity> {
    match read_owner_only_json(&record_path(blob)) {
        Ok(identity) => Some(identity),
        Err(error) => {
            // NotFound is the ordinary pre-upgrade/absent case; anything else
            // (corrupt JSON, permissions) means the fast path silently
            // degrades to a full hash every launch, so make it visible.
            if error.kind() != io::ErrorKind::NotFound {
                tracing::warn!(
                    blob = %blob.display(),
                    %error,
                    "package blob identity record unreadable; falling back to full hash"
                );
            }
            None
        }
    }
}

/// Persist the identity of `file` next to `blob`, owner-only like state.json.
pub(crate) fn store(blob: &Path, file: &fs::File) -> io::Result<()> {
    let identity = of(file)?;
    #[cfg(test)]
    if FAIL_NEXT_STORE.swap(false, std::sync::atomic::Ordering::Relaxed) {
        return Err(io::Error::other("injected identity write failure"));
    }
    write_owner_only_json(&record_path(blob), &identity).map_err(io::Error::other)
}

/// Test seam: inject a one-shot failure into the next `store` call, like
/// `fail_next_state_write` on `PackageStore`.
#[cfg(test)]
pub(crate) static FAIL_NEXT_STORE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(windows)]
fn platform_identity(file: &fs::File) -> io::Result<BlobIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        FileBasicInfo, FileIdInfo, GetFileInformationByHandleEx, FILE_BASIC_INFO, FILE_ID_INFO,
    };
    fn query<T>(
        handle: HANDLE,
        class: windows::Win32::Storage::FileSystem::FILE_INFO_BY_HANDLE_CLASS,
    ) -> io::Result<T> {
        let mut value = std::mem::MaybeUninit::<T>::uninit();
        unsafe {
            GetFileInformationByHandleEx(
                handle,
                class,
                value.as_mut_ptr().cast(),
                std::mem::size_of::<T>() as u32,
            )
        }
        .map_err(io::Error::other)?;
        Ok(unsafe { value.assume_init() })
    }
    let handle = HANDLE(file.as_raw_handle());
    let id: FILE_ID_INFO = query(handle, FileIdInfo)?;
    let basic: FILE_BASIC_INFO = query(handle, FileBasicInfo)?;
    if basic.ChangeTime == 0 {
        return Err(io::Error::other("filesystem reports no change time"));
    }
    Ok(BlobIdentity {
        volume_serial: id.VolumeSerialNumber,
        file_id: id
            .FileId
            .Identifier
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        size: file.metadata()?.len(),
        modified: basic.LastWriteTime as i128,
        changed: basic.ChangeTime as i128,
    })
}

#[cfg(unix)]
fn platform_identity(file: &fs::File) -> io::Result<BlobIdentity> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok(BlobIdentity {
        volume_serial: metadata.dev(),
        file_id: metadata.ino().to_string(),
        size: metadata.size(),
        modified: metadata.mtime() as i128 * 1_000_000_000 + metadata.mtime_nsec() as i128,
        changed: metadata.ctime() as i128 * 1_000_000_000 + metadata.ctime_nsec() as i128,
    })
}
