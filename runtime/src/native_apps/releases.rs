//! GitHub Releases probe + verified download for the hardcoded store apps.
//!
//! No GitHub API and no token: `GET <repo>/releases/latest/download/
//! SHA256SUMS.txt` answers 302 to `releases/download/<tag>/SHA256SUMS.txt`,
//! so the first redirect's `Location` carries the authoritative latest tag,
//! and the sums file's line for the host target carries the asset name +
//! sha256 that gate the download. The asset URL is rebuilt from the tag on
//! the configured base host — never taken from the redirect or the sums
//! file itself.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::NativeAppDescriptor;
use crate::package_store::MAX_ARCHIVE;

const MAX_SUMS: usize = 64 * 1024;
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("release endpoint unavailable")]
    Unavailable,
    #[error("release metadata is invalid: {0}")]
    Invalid(&'static str),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// A verified pointer to one downloadable asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub asset: String,
    pub sha256: String,
}

/// Result of a conditional latest-release fetch.
pub enum ReleaseCheck {
    /// Fresh metadata; `etag` pairs with the tag for the next poll.
    Fresh {
        info: ReleaseInfo,
        etag: Option<String>,
    },
    /// The cached etag is still current — keep the previous info.
    NotModified,
}

/// HTTP clients + base host for one batch of release operations. `probe`
/// never follows redirects (the tag lives in the first `Location` hop);
/// `http` follows them (the sums file and the asset land on the CDN).
pub struct ReleaseProbe {
    pub base: String,
    probe: reqwest::Client,
    http: reqwest::Client,
}

impl ReleaseProbe {
    pub fn new() -> Result<Self, ReleaseError> {
        Self::with_base(releases_base())
    }

    pub fn with_base(base: String) -> Result<Self, ReleaseError> {
        check_base(&base)?;
        let probe = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(CHECK_TIMEOUT)
            .user_agent(crate::brand::ENGINE_BINARY_STEM)
            .build()
            .map_err(|_| ReleaseError::Unavailable)?;
        let http = reqwest::Client::builder()
            .user_agent(crate::brand::ENGINE_BINARY_STEM)
            .build()
            .map_err(|_| ReleaseError::Unavailable)?;
        Ok(Self { base, probe, http })
    }
}

/// `https://github.com` unless `MUNDUS_APPS_RELEASES_BASE` overrides it —
/// the process-local escape hatch used by the E2E harness and unit tests.
pub fn releases_base() -> String {
    crate::brand::env("APPS_RELEASES_BASE")
        .map(|value| value.trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "https://github.com".to_owned())
}

/// The release host must be a bare HTTPS origin; debug/test builds may use
/// `http://` so a local stub can stand in for github.com.
fn check_base(base: &str) -> Result<reqwest::Url, ReleaseError> {
    let url = reqwest::Url::parse(base).map_err(|_| ReleaseError::Invalid("base url"))?;
    let scheme_ok = url.scheme() == "https" || (cfg!(debug_assertions) && url.scheme() == "http");
    if !scheme_ok
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
        || (url.path() != "/" && url.path() != "")
    {
        return Err(ReleaseError::Invalid("base url"));
    }
    Ok(url)
}

fn latest_sums_url(base: &str, repository: &str) -> String {
    format!("{base}/{repository}/releases/latest/download/SHA256SUMS.txt")
}

fn tagged_sums_url(base: &str, repository: &str, tag: &str) -> String {
    format!("{base}/{repository}/releases/download/{tag}/SHA256SUMS.txt")
}

pub fn asset_url(base: &str, repository: &str, tag: &str, asset: &str) -> String {
    format!("{base}/{repository}/releases/download/{tag}/{asset}")
}

include!("releases/parse.rs");

enum SumsGet {
    NotModified,
    Body { text: String, etag: Option<String> },
}

async fn get_sums(
    http: &reqwest::Client,
    url: &str,
    etag: Option<&str>,
) -> Result<SumsGet, ReleaseError> {
    let mut request = http.get(url).timeout(CHECK_TIMEOUT);
    if let Some(etag) = etag {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let response = request
        .send()
        .await
        .map_err(|_| ReleaseError::Unavailable)?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(SumsGet::NotModified);
    }
    if !response.status().is_success() {
        return Err(ReleaseError::Unavailable);
    }
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = response
        .bytes()
        .await
        .map_err(|_| ReleaseError::Unavailable)?;
    if bytes.len() > MAX_SUMS {
        return Err(ReleaseError::Invalid("sums too large"));
    }
    let text = String::from_utf8(bytes.to_vec()).map_err(|_| ReleaseError::Invalid("sums utf8"))?;
    Ok(SumsGet::Body { text, etag })
}

/// Latest release for `desc` on `target`. `etag` revalidates a cached sums
/// body — on `NotModified` the caller keeps its previous `ReleaseInfo`.
pub async fn fetch_latest(
    probe: &ReleaseProbe,
    desc: &NativeAppDescriptor,
    target: &str,
    etag: Option<&str>,
) -> Result<ReleaseCheck, ReleaseError> {
    let base_url = check_base(&probe.base)?;
    let response = probe
        .probe
        .get(latest_sums_url(&probe.base, desc.repository))
        .send()
        .await
        .map_err(|_| ReleaseError::Unavailable)?;
    if response.status().is_redirection() {
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(ReleaseError::Invalid("redirect location"))?;
        let tag = tag_from_location(&base_url, desc.repository, location)
            .ok_or(ReleaseError::Invalid("redirect location"))?;
        return match get_sums(
            &probe.http,
            &tagged_sums_url(&probe.base, desc.repository, &tag),
            etag,
        )
        .await?
        {
            SumsGet::NotModified => Ok(ReleaseCheck::NotModified),
            SumsGet::Body { text, etag } => Ok(ReleaseCheck::Fresh {
                info: info_from_sums(desc, Some(&tag), &text, target)?,
                etag,
            }),
        };
    }
    // A stub (or a proxy that pre-resolved the redirect) may serve the sums
    // directly — then the asset name alone carries the version.
    if response.status().is_success() {
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ReleaseError::Unavailable)?;
        if bytes.len() > MAX_SUMS {
            return Err(ReleaseError::Invalid("sums too large"));
        }
        let text =
            String::from_utf8(bytes.to_vec()).map_err(|_| ReleaseError::Invalid("sums utf8"))?;
        return Ok(ReleaseCheck::Fresh {
            info: info_from_sums(desc, None, &text, target)?,
            etag,
        });
    }
    Err(ReleaseError::Unavailable)
}

/// The pinned-tag variant of `fetch_latest` — install of a caller-chosen
/// version goes through the same sums-verified path.
pub async fn fetch_tagged(
    probe: &ReleaseProbe,
    desc: &NativeAppDescriptor,
    tag: &str,
    target: &str,
) -> Result<ReleaseInfo, ReleaseError> {
    match get_sums(
        &probe.http,
        &tagged_sums_url(&probe.base, desc.repository, tag),
        None,
    )
    .await?
    {
        SumsGet::NotModified => Err(ReleaseError::Unavailable),
        SumsGet::Body { text, .. } => info_from_sums(desc, Some(tag), &text, target),
    }
}

/// Stream an asset to `dest`, hashing as it goes. Returns the bytes' actual
/// sha256 + size; the caller compares against the sums line (and
/// `install_archive` re-verifies before extraction).
pub async fn download(
    probe: &ReleaseProbe,
    url: &str,
    dest: &Path,
) -> Result<(String, u64), ReleaseError> {
    let mut response = probe
        .http
        .get(url)
        .timeout(DOWNLOAD_TIMEOUT)
        .send()
        .await
        .map_err(|_| ReleaseError::Unavailable)?;
    if !response.status().is_success() {
        return Err(ReleaseError::Unavailable);
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_ARCHIVE)
    {
        return Err(ReleaseError::Invalid("archive too large"));
    }
    let mut file = std::fs::File::create(dest)?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ReleaseError::Unavailable)?
    {
        size = size.saturating_add(chunk.len() as u64);
        if size > MAX_ARCHIVE {
            let _ = std::fs::remove_file(dest);
            return Err(ReleaseError::Invalid("archive too large"));
        }
        hasher.update(&chunk);
        file.write_all(&chunk)?;
    }
    Ok((format!("{:x}", hasher.finalize()), size))
}
