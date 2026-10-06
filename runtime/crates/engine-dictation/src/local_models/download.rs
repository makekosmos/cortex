//! Resumable binary download with progress reporting for local model and
//! whisper.cpp runtime assets. Split out of `local_models` — this is the
//! transport half; model/runtime specifics live in the sibling modules.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use reqwest::header::{
    ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
};
use reqwest::Client;
use serde::Serialize;

use super::LocalModelsError;

const DOWNLOAD_MAX_ATTEMPTS: usize = 8;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub phase: &'static str,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f64>,
}

pub type ProgressCallback<'a> = dyn FnMut(DownloadProgress) + Send + 'a;

/// Runtime archives are pinned GitHub release assets; a redirect anywhere
/// else means the response is not the asset the sha256 was published for.
#[cfg(windows)]
fn trusted_runtime_response(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("github.com" | "release-assets.githubusercontent.com")
        )
}

pub(super) async fn download_file(
    client: &Client,
    url: &str,
    destination: &Path,
    phase: &'static str,
    max_bytes: Option<u64>,
    progress: &mut ProgressCallback<'_>,
) -> Result<(), LocalModelsError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let part_path = destination.with_extension("part");
    let request_url = url.to_string();
    let mut last_error = None;

    for attempt in 1..=DOWNLOAD_MAX_ATTEMPTS {
        let mut downloaded_bytes = part_path.metadata().map(|m| m.len()).unwrap_or(0);
        if max_bytes.is_some_and(|limit| downloaded_bytes > limit) {
            fs::remove_file(&part_path)?;
            downloaded_bytes = 0;
        }
        let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
        if downloaded_bytes > 0 {
            request = request.header(RANGE, format!("bytes={downloaded_bytes}-"));
        }

        let mut response = match request.send().await {
            Ok(response) => response,
            Err(e) => {
                last_error = Some(format!("{request_url}: {e}"));
                sleep_download_retry(attempt).await;
                continue;
            }
        };
        let status = response.status();
        let final_url = response.url().to_string();
        #[cfg(windows)]
        if phase == "tool" && !trusted_runtime_response(response.url()) {
            return Err(LocalModelsError::Download(format!(
                "{request_url}: untrusted runtime redirect"
            )));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let content_encoding = response
            .headers()
            .get(CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let content_range = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        tracing::info!(
            phase,
            attempt,
            status = %status,
            resumed_from = downloaded_bytes,
            url = %request_url,
            final_url = %final_url,
            content_type = %content_type,
            content_encoding = %content_encoding,
            content_range = %content_range,
            "dictation local model download response"
        );
        if downloaded_bytes > 0 && status == reqwest::StatusCode::OK {
            tracing::warn!(
                phase,
                attempt,
                resumed_from = downloaded_bytes,
                url = %request_url,
                "dictation local model download server ignored range; restarting"
            );
            let _ = fs::remove_file(&part_path);
            downloaded_bytes = 0;
        } else if downloaded_bytes > 0 && status != reqwest::StatusCode::PARTIAL_CONTENT {
            last_error = Some(format!(
                "{request_url}: resume failed with HTTP {status} ({content_type})"
            ));
            sleep_download_retry(attempt).await;
            continue;
        }
        if !status.is_success() {
            return Err(LocalModelsError::Download(format!(
                "{request_url}: HTTP {status} ({content_type})"
            )));
        }
        let lower_content_type = content_type.to_ascii_lowercase();
        if lower_content_type.contains("text/html")
            || lower_content_type.contains("application/json")
            || lower_content_type.starts_with("text/")
        {
            return Err(LocalModelsError::Download(format!(
                "{request_url}: expected binary download, got {content_type}"
            )));
        }
        let total_bytes = parse_content_range_total(&content_range)
            .or_else(|| {
                response
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(|remaining| downloaded_bytes.saturating_add(remaining))
            })
            .or_else(|| {
                response
                    .content_length()
                    .map(|remaining| downloaded_bytes + remaining)
            });
        let mut file = OpenOptions::new()
            .create(true)
            .append(downloaded_bytes > 0)
            .write(true)
            .truncate(downloaded_bytes == 0)
            .open(&part_path)?;
        progress(DownloadProgress {
            phase,
            downloaded_bytes,
            total_bytes,
            percent: progress_percent(downloaded_bytes, total_bytes),
        });
        let mut advanced = false;
        loop {
            let chunk = match response.chunk().await {
                Ok(chunk) => chunk,
                Err(e) => {
                    let message = format!(
                        "{request_url}: stream read failed after {downloaded_bytes} bytes \
                             ({content_type}, encoding {content_encoding}): {e}"
                    );
                    tracing::warn!(
                        phase,
                        attempt,
                        error = %message,
                        "dictation local model download chunk failed",
                    );
                    last_error = Some(message);
                    break;
                }
            };
            let Some(chunk) = chunk else {
                file.flush()?;
                fs::rename(&part_path, destination)?;
                return Ok(());
            };
            if max_bytes.is_some_and(|limit| downloaded_bytes + chunk.len() as u64 > limit) {
                drop(file);
                let _ = fs::remove_file(&part_path);
                return Err(LocalModelsError::Download(format!(
                    "{request_url}: download exceeds verified size limit"
                )));
            }
            file.write_all(&chunk)?;
            advanced = true;
            downloaded_bytes += chunk.len() as u64;
            progress(DownloadProgress {
                phase,
                downloaded_bytes,
                total_bytes,
                percent: progress_percent(downloaded_bytes, total_bytes),
            });
        }
        file.flush()?;
        if !advanced && attempt == DOWNLOAD_MAX_ATTEMPTS {
            break;
        }
        sleep_download_retry(attempt).await;
    }

    Err(LocalModelsError::Download(last_error.unwrap_or_else(
        || format!("{request_url}: download failed"),
    )))
}

fn parse_content_range_total(value: &str) -> Option<u64> {
    value
        .rsplit_once('/')
        .and_then(|(_, total)| total.parse::<u64>().ok())
}

fn progress_percent(downloaded_bytes: u64, total_bytes: Option<u64>) -> Option<f64> {
    total_bytes
        .filter(|total| *total > 0)
        .map(|total| (downloaded_bytes as f64 / total as f64 * 100.0).min(100.0))
}

async fn sleep_download_retry(attempt: usize) {
    if attempt >= DOWNLOAD_MAX_ATTEMPTS {
        return;
    }
    let delay_ms = (250_u64 * 2_u64.saturating_pow(attempt.saturating_sub(1) as u32)).min(5_000);
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::Method::GET;
    use httpmock::MockServer;

    #[tokio::test]
    async fn download_file_streams_binary_and_reports_progress() {
        let server = MockServer::start_async().await;
        let body = b"local model bytes";
        server
            .mock_async(|when, then| {
                when.method(GET).path("/model.bin");
                then.status(200)
                    .header("content-type", "application/octet-stream")
                    .header("content-length", body.len().to_string())
                    .body(body.as_slice());
            })
            .await;

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("model.bin");
        let client = engine_base::http::client();
        let mut events = Vec::new();

        download_file(
            &client,
            &server.url("/model.bin"),
            &path,
            "model",
            None,
            &mut |progress| events.push(progress),
        )
        .await
        .expect("download");

        assert_eq!(fs::read(&path).expect("downloaded file"), body);
        assert!(
            !path.with_extension("part").exists(),
            "part file should be renamed away"
        );
        assert!(
            events.iter().any(|event| event.downloaded_bytes == 0
                && event.total_bytes == Some(body.len() as u64)
                && event.percent == Some(0.0)),
            "missing initial progress event: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| event.downloaded_bytes == body.len() as u64
                    && event.total_bytes == Some(body.len() as u64)
                    && event.percent == Some(100.0)),
            "missing complete progress event: {events:?}"
        );
    }

    #[tokio::test]
    async fn download_file_rejects_html_response() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/model.bin");
                then.status(200)
                    .header("content-type", "text/html; charset=utf-8")
                    .body("<html>login</html>");
            })
            .await;

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("model.bin");
        let client = engine_base::http::client();
        let err = download_file(
            &client,
            &server.url("/model.bin"),
            &path,
            "model",
            None,
            &mut |_| {},
        )
        .await
        .expect_err("html response must fail");

        assert!(
            err.to_string().contains("expected binary download"),
            "unexpected error: {err}"
        );
        assert!(!path.exists(), "html response must not be stored as model");
    }

    #[tokio::test]
    async fn download_file_enforces_hard_size_limit() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/oversized.bin");
                then.status(200)
                    .header("content-type", "application/octet-stream")
                    .body(vec![7_u8; 17]);
            })
            .await;
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("oversized.bin");

        let error = download_file(
            &engine_base::http::client(),
            &server.url("/oversized.bin"),
            &path,
            "model",
            Some(16),
            &mut |_| {},
        )
        .await
        .expect_err("oversized body must fail");

        assert!(error.to_string().contains("size limit"));
        assert!(!path.exists());
        assert!(!path.with_extension("part").exists());
    }

    #[tokio::test]
    async fn download_file_resumes_existing_part_with_range_request() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/model.bin")
                    .header("range", "bytes=6-");
                then.status(206)
                    .header("content-type", "application/octet-stream")
                    .header("content-range", "bytes 6-10/11")
                    .header("content-length", "5")
                    .body("world");
            })
            .await;

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("model.bin");
        fs::write(path.with_extension("part"), b"hello ").expect("part file");
        let client = engine_base::http::client();
        let mut events = Vec::new();

        download_file(
            &client,
            &server.url("/model.bin"),
            &path,
            "model",
            None,
            &mut |progress| events.push(progress),
        )
        .await
        .expect("download");

        assert_eq!(fs::read(&path).expect("downloaded file"), b"hello world");
        assert!(
            events
                .iter()
                .any(|event| event.downloaded_bytes == 6 && event.total_bytes == Some(11)),
            "missing resumed initial progress event: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|event| event.downloaded_bytes == 11 && event.percent == Some(100.0)),
            "missing resumed complete progress event: {events:?}"
        );
    }
}
