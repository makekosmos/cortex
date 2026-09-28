use std::path::Path;

use base64::Engine as _;
use sha2::{Digest, Sha512};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::UpdaterError;

const PROGRESS_INTERVAL: u64 = 1024 * 1024;

pub(crate) async fn download_resumable(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_size: u64,
    mut on_progress: impl FnMut(u64, u64) + Send,
) -> Result<(), UpdaterError> {
    let mut existing = tokio::fs::metadata(dest)
        .await
        .map(|meta| meta.len())
        .unwrap_or(0);
    if existing > expected_size {
        existing = 0;
    }
    if existing == expected_size {
        on_progress(existing, expected_size);
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(io_error)?;
    }

    let mut request = client.get(url);
    if existing > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={existing}-"));
    }
    let mut response = request.send().await.map_err(network_error)?;
    if !response.status().is_success() {
        return Err(UpdaterError::Network(format!(
            "GET {url}: HTTP {}",
            response.status()
        )));
    }
    let restart = existing > 0 && response.status() != reqwest::StatusCode::PARTIAL_CONTENT;
    if restart {
        existing = 0;
    }
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!restart && existing > 0)
        .truncate(restart || existing == 0)
        .open(dest)
        .await
        .map_err(io_error)?;

    let mut since_report = 0;
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        file.write_all(&chunk).await.map_err(io_error)?;
        existing += chunk.len() as u64;
        since_report += chunk.len() as u64;
        if since_report >= PROGRESS_INTERVAL || existing >= expected_size {
            since_report = 0;
            on_progress(existing, expected_size);
        }
    }
    file.sync_all().await.map_err(io_error)?;
    on_progress(existing, expected_size);
    Ok(())
}

pub(crate) async fn verify_file(
    path: &Path,
    expected_size: u64,
    expected_sha512_b64: &str,
) -> Result<(), UpdaterError> {
    let metadata = tokio::fs::metadata(path).await.map_err(io_error)?;
    if metadata.len() != expected_size {
        return Err(UpdaterError::SizeMismatch {
            expected: expected_size,
            actual: metadata.len(),
        });
    }
    let expected = base64::engine::general_purpose::STANDARD
        .decode(expected_sha512_b64)
        .map_err(|_| UpdaterError::MalformedManifest)?;
    let mut file = tokio::fs::File::open(path).await.map_err(io_error)?;
    let mut hasher = Sha512::new();
    let mut buffer = vec![0; 256 * 1024];
    loop {
        let read = file.read(&mut buffer).await.map_err(io_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    if hasher.finalize().as_slice() == expected.as_slice() {
        Ok(())
    } else {
        Err(UpdaterError::HashMismatch)
    }
}

pub(crate) async fn verify_and_commit(
    part_path: &Path,
    final_path: &Path,
    expected_size: u64,
    expected_sha512_b64: &str,
) -> Result<(), UpdaterError> {
    verify_file(part_path, expected_size, expected_sha512_b64).await?;
    if tokio::fs::try_exists(final_path).await.map_err(io_error)? {
        tokio::fs::remove_file(final_path).await.map_err(io_error)?;
    }
    tokio::fs::rename(part_path, final_path)
        .await
        .map_err(io_error)
}

fn io_error(error: std::io::Error) -> UpdaterError {
    UpdaterError::Io(error.to_string())
}

fn network_error(error: reqwest::Error) -> UpdaterError {
    UpdaterError::Network(error.to_string())
}

#[cfg(test)]
#[path = "download_tests.rs"]
mod tests;
