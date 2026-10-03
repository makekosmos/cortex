//! Shared streaming file-digest and size verification — the single
//! implementation used by the self-updater, native app installs, the
//! installer manifest checks, the package store and the dictation runtime
//! installer. Trust model: HTTPS + GitHub Releases + a pinned hash/size, so
//! every download path must run through this file instead of growing its
//! own hashing loop.

use std::fs;
use std::io::{self, Read};
use std::path::Path;

use sha2::digest::Output;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::lock_file::retry_io;
use crate::package_store::eq_hash;

#[derive(Debug, Error)]
pub(crate) enum VerifyError {
    #[error("size mismatch: expected {expected} bytes, got {actual}")]
    Size { expected: u64, actual: u64 },
    #[error("sha256 mismatch: expected {expected}, got {actual}")]
    Hash { expected: String, actual: String },
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

/// Streamed digest — never buffers the whole file.
pub(crate) fn file_digest<D: Digest>(path: &Path) -> io::Result<Output<D>> {
    let mut file = retry_io(|| fs::File::open(path))?;
    let mut hasher = D::new();
    let mut buffer = [0; 256 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize())
}

/// sha256 of an already-open reader — same loop for callers that stream
/// from a zip entry or a `File` they hold themselves.
pub(crate) fn sha256_reader(mut reader: impl Read) -> io::Result<String> {
    let mut hasher = Sha256::new();
    io::copy(&mut reader, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Lowercase hex sha256 of a file.
pub(crate) fn file_sha256(path: &Path) -> io::Result<String> {
    Ok(format!("{:x}", file_digest::<Sha256>(path)?))
}

/// Hash-only check for downloads whose exact byte size is not pinned.
pub(crate) fn verify_sha256(path: &Path, expected_sha256: &str) -> Result<(), VerifyError> {
    let actual = file_sha256(path)?;
    if eq_hash(&actual, expected_sha256) {
        Ok(())
    } else {
        Err(VerifyError::Hash {
            expected: expected_sha256.to_owned(),
            actual,
        })
    }
}

/// A downloaded artifact is trusted only when both the pinned byte size and
/// the pinned sha256 match — the size check runs first because it is cheap
/// and rejects truncated or padded files before hashing.
pub(crate) fn verify_size_and_sha256(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), VerifyError> {
    let actual = fs::metadata(path)?.len();
    if actual != expected_size {
        return Err(VerifyError::Size {
            expected: expected_size,
            actual,
        });
    }
    verify_sha256(path, expected_sha256)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_accepts_matching_size_and_sha256() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("asset.bin");
        let body = b"pinned bytes";
        fs::write(&path, body).expect("asset");
        let sha = file_sha256(&path).expect("sha256");

        verify_size_and_sha256(&path, body.len() as u64, &sha).expect("verify");
    }

    #[test]
    fn verify_rejects_wrong_size_before_hashing() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("asset.bin");
        fs::write(&path, b"pinned bytes").expect("asset");
        let sha = file_sha256(&path).expect("sha256");

        let error = verify_size_and_sha256(&path, 1, &sha).expect_err("size must fail");
        assert!(matches!(
            error,
            VerifyError::Size {
                expected: 1,
                actual: 12
            }
        ));
    }

    #[test]
    fn verify_rejects_wrong_sha256() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("asset.bin");
        fs::write(&path, b"pinned bytes").expect("asset");

        let error = verify_size_and_sha256(&path, 12, &"0".repeat(64)).expect_err("hash must fail");
        assert!(matches!(error, VerifyError::Hash { .. }));
        assert!(verify_sha256(&path, &"0".repeat(64)).is_err());
    }

    #[test]
    fn sha256_reader_hashes_any_stream() {
        let digest = sha256_reader(io::Cursor::new(b"abc")).expect("digest");
        assert_eq!(digest, file_sha256_buf(b"abc"));
    }

    fn file_sha256_buf(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }
}
