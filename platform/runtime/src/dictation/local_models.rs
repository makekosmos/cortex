use std::fs::{self, OpenOptions};
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{
    ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
};
use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::config;

const MODELS_DIR: &str = "models/dictation";
const TOOLS_DIR: &str = "tools/dictation/whisper.cpp";
const VAD_DIR: &str = "tools/dictation/vad";
const VAD_MODEL_FILENAME: &str = "ggml-silero-v6.2.0.bin";
const VAD_MODEL_URL: &str =
    "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin";
const VAD_MODEL_SHA256: &str = "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987";
const DOWNLOAD_MAX_ATTEMPTS: usize = 8;

#[cfg(windows)]
const WHISPER_CPP_ZIP_URL: &str =
    "https://github.com/ggml-org/whisper.cpp/releases/download/v1.8.4/whisper-bin-x64.zip";

#[derive(Debug, Clone, Copy)]
pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub sha256: Option<&'static str>,
    pub size_mb: u64,
    pub accuracy_score: f32,
    pub speed_score: f32,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    pub url: String,
    pub size_mb: u64,
    pub accuracy_score: f32,
    pub speed_score: f32,
    pub recommended: bool,
    pub downloaded: bool,
    pub selected: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelsSnapshot {
    pub models_dir: String,
    pub command_path: Option<String>,
    pub command_installed: bool,
    pub models: Vec<LocalModelInfo>,
}

#[derive(Debug, Error)]
pub enum LocalModelsError {
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("model is not downloaded: {0}")]
    ModelNotDownloaded(String),
    #[error("downloading local models is not supported on this platform yet")]
    UnsupportedPlatform,
    #[error("download failed: {0}")]
    Download(String),
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("sha256 mismatch for {path}: expected {expected}, got {actual}")]
    ShaMismatch {
        path: String,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub phase: &'static str,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f64>,
}

pub type ProgressCallback<'a> = dyn FnMut(DownloadProgress) + Send + 'a;

pub const MODEL_CATALOG: &[ModelSpec] = &[
    ModelSpec {
        id: "tiny-q5_1",
        name: "Whisper Tiny",
        description: "Самая быстрая проверочная модель. Подходит для smoke-теста, качество ниже.",
        filename: "ggml-tiny-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny-q5_1.bin",
        sha256: None,
        size_mb: 31,
        accuracy_score: 0.35,
        speed_score: 0.98,
        recommended: false,
    },
    ModelSpec {
        id: "small",
        name: "Whisper Small",
        description: "Быстрая и достаточно точная модель для повседневной диктовки.",
        filename: "ggml-small.bin",
        url: "https://blob.handy.computer/ggml-small.bin",
        sha256: Some("1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b"),
        size_mb: 465,
        accuracy_score: 0.60,
        speed_score: 0.85,
        recommended: true,
    },
    ModelSpec {
        id: "medium",
        name: "Whisper Medium",
        description: "Выше качество, заметно тяжелее для CPU.",
        filename: "whisper-medium-q4_1.bin",
        url: "https://blob.handy.computer/whisper-medium-q4_1.bin",
        sha256: Some("79283fc1f9fe12ca3248543fbd54b73292164d8df5a16e095e2bceeaaabddf57"),
        size_mb: 469,
        accuracy_score: 0.75,
        speed_score: 0.60,
        recommended: false,
    },
    ModelSpec {
        id: "turbo",
        name: "Whisper Large v3 Turbo",
        description: "Лучший баланс качества и скорости для сильной машины.",
        filename: "ggml-large-v3-turbo.bin",
        url: "https://blob.handy.computer/ggml-large-v3-turbo.bin",
        sha256: Some("1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69"),
        size_mb: 1549,
        accuracy_score: 0.80,
        speed_score: 0.40,
        recommended: false,
    },
    ModelSpec {
        id: "large",
        name: "Whisper Large v3 q5",
        description: "Самое высокое качество из локального списка, но медленнее.",
        filename: "ggml-large-v3-q5_0.bin",
        url: "https://blob.handy.computer/ggml-large-v3-q5_0.bin",
        sha256: Some("d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1"),
        size_mb: 1031,
        accuracy_score: 0.85,
        speed_score: 0.30,
        recommended: false,
    },
];

fn model_spec(model_id: &str) -> Result<&'static ModelSpec, LocalModelsError> {
    MODEL_CATALOG
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| LocalModelsError::ModelNotFound(model_id.to_owned()))
}

pub fn models_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(MODELS_DIR)
}

pub fn tools_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(TOOLS_DIR)
}

pub fn vad_model_path(data_dir: &Path) -> PathBuf {
    data_dir.join(VAD_DIR).join(VAD_MODEL_FILENAME)
}

pub fn model_path(data_dir: &Path, spec: &ModelSpec) -> PathBuf {
    models_dir(data_dir).join(spec.filename)
}

pub fn model_path_by_id(data_dir: &Path, model_id: &str) -> Result<PathBuf, LocalModelsError> {
    Ok(model_path(data_dir, model_spec(model_id)?))
}

pub fn command_path(data_dir: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        Some(tools_dir(data_dir).join("Release").join("whisper-cli.exe"))
    }
    #[cfg(not(windows))]
    {
        let _ = data_dir;
        None
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub fn snapshot(data_dir: &Path, cfg: &config::DictationConfig) -> LocalModelsSnapshot {
    let models_dir = models_dir(data_dir);
    let command_path = command_path(data_dir);
    let command_installed = command_path.as_ref().is_some_and(|path| path.is_file());
    let selected_path = cfg.local_model_path.as_deref();
    let models = MODEL_CATALOG
        .iter()
        .map(|spec| {
            let path = model_path(data_dir, spec);
            let path_text = path_string(&path);
            LocalModelInfo {
                id: spec.id.to_owned(),
                name: spec.name.to_owned(),
                description: spec.description.to_owned(),
                filename: spec.filename.to_owned(),
                url: spec.url.to_owned(),
                size_mb: spec.size_mb,
                accuracy_score: spec.accuracy_score,
                speed_score: spec.speed_score,
                recommended: spec.recommended,
                downloaded: path.is_file(),
                selected: cfg.local_model.as_deref() == Some(spec.id)
                    || selected_path == Some(path_text.as_str()),
                path: path.is_file().then_some(path_text),
            }
        })
        .collect();

    LocalModelsSnapshot {
        models_dir: path_string(&models_dir),
        command_path: command_path.map(|path| path_string(&path)),
        command_installed,
        models,
    }
}

async fn download_file(
    client: &Client,
    url: &str,
    destination: &Path,
    phase: &'static str,
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
                        "{request_url}: stream read failed after {downloaded_bytes} bytes ({content_type}, encoding {content_encoding}): {e}"
                    );
                    tracing::warn!(phase, attempt, error = %message, "dictation local model download chunk failed");
                    last_error = Some(message);
                    break;
                }
            };
            let Some(chunk) = chunk else {
                file.flush()?;
                fs::rename(&part_path, destination)?;
                return Ok(());
            };
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

fn verify_sha256(path: &Path, expected: &str) -> Result<(), LocalModelsError> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = format!("{:x}", hasher.finalize());
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(LocalModelsError::ShaMismatch {
            path: path_string(path),
            expected: expected.to_owned(),
            actual,
        })
    }
}

pub async fn ensure_model(
    client: &Client,
    data_dir: &Path,
    model_id: &str,
) -> Result<PathBuf, LocalModelsError> {
    ensure_model_with_progress(client, data_dir, model_id, &mut |_| {}).await
}

pub async fn ensure_model_with_progress(
    client: &Client,
    data_dir: &Path,
    model_id: &str,
    progress: &mut ProgressCallback<'_>,
) -> Result<PathBuf, LocalModelsError> {
    let spec = model_spec(model_id)?;
    let path = model_path(data_dir, spec);
    if !path.is_file() {
        download_file(client, spec.url, &path, "model", progress).await?;
    }
    if let Some(expected) = spec.sha256 {
        verify_sha256(&path, expected)?;
    }
    Ok(path)
}

pub async fn ensure_vad_model(
    client: &Client,
    data_dir: &Path,
) -> Result<PathBuf, LocalModelsError> {
    let path = vad_model_path(data_dir);
    if !path.is_file() {
        download_file(client, VAD_MODEL_URL, &path, "vad", &mut |_| {}).await?;
    }
    verify_sha256(&path, VAD_MODEL_SHA256)?;
    Ok(path)
}

pub fn delete_model(data_dir: &Path, model_id: &str) -> Result<PathBuf, LocalModelsError> {
    let path = model_path_by_id(data_dir, model_id)?;
    if path.is_file() {
        fs::remove_file(&path)?;
    }
    let part_path = path.with_extension("part");
    if part_path.is_file() {
        fs::remove_file(&part_path)?;
    }
    Ok(path)
}

#[cfg(windows)]
pub async fn ensure_whisper_cpp(
    client: &Client,
    data_dir: &Path,
) -> Result<PathBuf, LocalModelsError> {
    ensure_whisper_cpp_with_progress(client, data_dir, &mut |_| {}).await
}

#[cfg(windows)]
pub async fn ensure_whisper_cpp_with_progress(
    client: &Client,
    data_dir: &Path,
    progress: &mut ProgressCallback<'_>,
) -> Result<PathBuf, LocalModelsError> {
    let command = command_path(data_dir).ok_or(LocalModelsError::UnsupportedPlatform)?;
    if command.is_file() {
        return Ok(command);
    }
    let dir = tools_dir(data_dir);
    fs::create_dir_all(&dir)?;
    let archive_path = dir.join("whisper-bin-x64.zip");
    download_file(client, WHISPER_CPP_ZIP_URL, &archive_path, "tool", progress).await?;
    progress(DownloadProgress {
        phase: "extract",
        downloaded_bytes: 0,
        total_bytes: None,
        percent: None,
    });
    let bytes = fs::read(&archive_path)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    archive.extract(&dir)?;
    let _ = fs::remove_file(&archive_path);
    if command.is_file() {
        Ok(command)
    } else {
        Err(LocalModelsError::Download(format!(
            "whisper.cpp archive did not contain {}",
            path_string(&command)
        )))
    }
}

#[cfg(not(windows))]
pub async fn ensure_whisper_cpp(
    _client: &Client,
    _data_dir: &Path,
) -> Result<PathBuf, LocalModelsError> {
    Err(LocalModelsError::UnsupportedPlatform)
}

#[cfg(not(windows))]
pub async fn ensure_whisper_cpp_with_progress(
    _client: &Client,
    _data_dir: &Path,
    _progress: &mut ProgressCallback<'_>,
) -> Result<PathBuf, LocalModelsError> {
    Err(LocalModelsError::UnsupportedPlatform)
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
                    .header("content-length", &body.len().to_string())
                    .body(body.as_slice());
            })
            .await;

        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("model.bin");
        let client = Client::new();
        let mut events = Vec::new();

        download_file(
            &client,
            &server.url("/model.bin"),
            &path,
            "model",
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
        let client = Client::new();
        let err = download_file(
            &client,
            &server.url("/model.bin"),
            &path,
            "model",
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
        let client = Client::new();
        let mut events = Vec::new();

        download_file(
            &client,
            &server.url("/model.bin"),
            &path,
            "model",
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
