//! GitHub Releases probe + verified download for the hardcoded store apps.
//!
//! No GitHub API and no token: `GET <repo>/releases/latest/download/
//! SHA256SUMS.txt` answers 302 to `releases/download/<tag>/SHA256SUMS.txt`,
//! so the first redirect's `Location` carries the authoritative latest tag,
//! and the sums file's line for the host target carries the asset name +
//! sha256 that gate the download. The asset URL is rebuilt from the tag on
//! the configured base host — never taken from the redirect or the sums
//! file itself.

use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::AsyncWriteExt;

use super::NativeAppDescriptor;
use crate::package_store::MAX_ARCHIVE;

const MAX_SUMS: usize = 64 * 1024;
/// Per-request connect/response budget for the release probe.
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
/// No single response body may stall longer than this; a slow but moving
/// link is allowed to take as long as it needs.
const IO_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum ReleaseError {
    /// Transport failure or a non-2xx answer — the host is offline or the
    /// release endpoint is down. Maps to the `offline` apps error code.
    #[error("release endpoint unavailable")]
    Unavailable,
    /// The endpoint answered but violated the protocol: bad redirect,
    /// oversized or unparseable sums, tag/asset version disagreement,
    /// oversized download. Maps to `integrity`/`invalid`.
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
#[derive(Debug)]
pub enum ReleaseCheck {
    /// Fresh metadata; `etag` pairs with the tag for the next poll.
    Fresh {
        info: ReleaseInfo,
        etag: Option<String>,
    },
    /// The cached etag is still current — keep the previous info.
    NotModified,
}

/// One previously-fetched latest release, for conditional revalidation.
/// `tag` pins the etag: a `304` counts only when the redirect still points
/// at the tag the etag came from (otherwise the store would silently keep
/// serving an older version as "latest").
pub struct CachedReleaseMeta {
    pub tag: String,
    pub etag: String,
}

/// HTTP clients + base host for one batch of release operations. `probe`
/// never follows redirects (the tag lives in the first `Location` hop);
/// `http` follows them (the sums file and the asset land on the CDN — the
/// final hop's scheme is still checked).
pub struct ReleaseProbe {
    /// Validated bare origin (checked once at construction).
    pub(crate) base: reqwest::Url,
    probe: reqwest::Client,
    http: reqwest::Client,
}

impl ReleaseProbe {
    pub fn new() -> Result<Self, ReleaseError> {
        Self::with_base(releases_base())
    }

    pub fn with_base(base: String) -> Result<Self, ReleaseError> {
        let base = check_base(&base)?;
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
/// the process-local escape hatch for dev installs pointed at a stub; unit
/// tests pass the stub URL to `ReleaseProbe::with_base` instead.
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
    if !url_scheme_ok(url.scheme())
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

/// HTTPS for release builds; a debug build may talk plain http to a local
/// stub. Applied to every URL we fetch — including the URL a redirect chain
/// lands on, so a man-in-the-middle cannot downgrade the hop.
fn url_scheme_ok(scheme: &str) -> bool {
    scheme == "https" || (cfg!(debug_assertions) && scheme == "http")
}

fn check_response_url(url: &reqwest::Url) -> Result<(), ReleaseError> {
    if url_scheme_ok(url.scheme()) {
        Ok(())
    } else {
        Err(ReleaseError::Invalid("insecure redirect target"))
    }
}

fn latest_sums_url(base: &reqwest::Url, repository: &str) -> reqwest::Url {
    url_path(
        base,
        &format!("{repository}/releases/latest/download/SHA256SUMS.txt"),
    )
}

fn tagged_sums_url(base: &reqwest::Url, repository: &str, tag: &str) -> reqwest::Url {
    url_path(
        base,
        &format!("{repository}/releases/download/{tag}/SHA256SUMS.txt"),
    )
}

pub fn asset_url(base: &reqwest::Url, repository: &str, tag: &str, asset: &str) -> reqwest::Url {
    url_path(
        base,
        &format!("{repository}/releases/download/{tag}/{asset}"),
    )
}

/// Join `path` onto a bare-origin base. `check_base` guarantees the base is
/// exactly an origin, so the leading-slash join is unambiguous.
fn url_path(base: &reqwest::Url, path: &str) -> reqwest::Url {
    base.join(path).unwrap_or_else(|_| base.clone())
}

include!("releases/parse.rs");

enum SumsGet {
    NotModified,
    Body { text: String, etag: Option<String> },
}

/// Read a sums response body with the size cap enforced while streaming —
/// an oversized body must not be buffered whole.
async fn sums_body(response: reqwest::Response) -> Result<(String, Option<String>), ReleaseError> {
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut body = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ReleaseError::Unavailable)?
    {
        if body.len() + chunk.len() > MAX_SUMS {
            return Err(ReleaseError::Invalid("sums too large"));
        }
        body.extend_from_slice(&chunk);
    }
    let text = String::from_utf8(body).map_err(|_| ReleaseError::Invalid("sums utf8"))?;
    Ok((text, etag))
}

async fn get_sums(
    http: &reqwest::Client,
    url: &reqwest::Url,
    etag: Option<&str>,
) -> Result<SumsGet, ReleaseError> {
    let mut request = http.get(url.clone()).timeout(CHECK_TIMEOUT);
    if let Some(etag) = etag {
        request = request.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let response = request.send().await.map_err(|error| {
        tracing::debug!(target: "native_apps", %error, "sums fetch failed");
        ReleaseError::Unavailable
    })?;
    check_response_url(response.url())?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(SumsGet::NotModified);
    }
    if !response.status().is_success() {
        return Err(ReleaseError::Unavailable);
    }
    let (text, etag) = sums_body(response).await?;
    Ok(SumsGet::Body { text, etag })
}

/// Latest release for `desc` on `target`. `cached` revalidates a stored
/// sums body — a `304` counts only when the redirect still lands on the
/// cached tag; a moved redirect refetches unconditionally.
pub async fn fetch_latest(
    probe: &ReleaseProbe,
    desc: &NativeAppDescriptor,
    target: &str,
    cached: Option<&CachedReleaseMeta>,
) -> Result<ReleaseCheck, ReleaseError> {
    let response = probe
        .probe
        .get(latest_sums_url(&probe.base, desc.repository))
        .send()
        .await
        .map_err(|error| {
            tracing::debug!(target: "native_apps", %error, "latest release probe failed");
            ReleaseError::Unavailable
        })?;
    if !response.status().is_redirection() {
        // The release endpoint must redirect to the tagged sums — a direct
        // body has no authoritative tag to verify against.
        return Err(ReleaseError::Unavailable);
    }
    let location = response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(ReleaseError::Invalid("redirect location"))?;
    let tag = tag_from_location(&probe.base, desc.repository, location)
        .ok_or(ReleaseError::Invalid("redirect location"))?;
    let sums = get_sums(
        &probe.http,
        &tagged_sums_url(&probe.base, desc.repository, &tag),
        cached.map(|cached| cached.etag.as_str()),
    )
    .await?;
    match sums {
        SumsGet::NotModified if cached.is_some_and(|cached| cached.tag == tag) => {
            Ok(ReleaseCheck::NotModified)
        }
        // A 304 for a different tag means the release moved while the sums
        // body is byte-identical — the etag answered for the old tag, so it
        // proves nothing about this one. Refetch unconditionally once.
        SumsGet::NotModified => {
            match get_sums(
                &probe.http,
                &tagged_sums_url(&probe.base, desc.repository, &tag),
                None,
            )
            .await?
            {
                SumsGet::NotModified => Err(ReleaseError::Invalid("etag without body")),
                SumsGet::Body { text, etag } => Ok(ReleaseCheck::Fresh {
                    info: info_from_sums(desc, &tag, &text, target)?,
                    etag,
                }),
            }
        }
        SumsGet::Body { text, etag } => Ok(ReleaseCheck::Fresh {
            info: info_from_sums(desc, &tag, &text, target)?,
            etag,
        }),
    }
}

/// A downloaded asset that deletes itself on drop — a cancelled or failed
/// install can never leave a partial `.download-*` file behind.
pub struct TempDownload {
    path: std::path::PathBuf,
}

impl TempDownload {
    pub fn at(path: std::path::PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDownload {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Stream an asset to `dest`, hashing as it goes. Returns the bytes' actual
/// sha256 + size; the caller compares against the sums line (and
/// `install_archive` re-verifies before extraction). `progress` receives
/// (downloaded, content-length) per chunk for the Store's progress row.
pub async fn download(
    probe: &ReleaseProbe,
    url: &reqwest::Url,
    dest: &Path,
    progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
) -> Result<(String, u64), ReleaseError> {
    let mut response = probe
        .http
        .get(url.clone())
        .timeout(CHECK_TIMEOUT)
        .send()
        .await
        .map_err(|error| {
            tracing::debug!(target: "native_apps", %error, "asset download failed");
            ReleaseError::Unavailable
        })?;
    check_response_url(response.url())?;
    if !response.status().is_success() {
        return Err(ReleaseError::Unavailable);
    }
    let total = response.content_length();
    if total.is_some_and(|len| len > MAX_ARCHIVE) {
        return Err(ReleaseError::Invalid("archive too large"));
    }
    // Async file I/O — a multi-MB stream must not block a tokio worker.
    let mut file = tokio::fs::File::create(dest).await?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    while let Some(chunk) = tokio::time::timeout(IO_IDLE_TIMEOUT, response.chunk())
        .await
        .map_err(|_| ReleaseError::Unavailable)?
        .map_err(|_| ReleaseError::Unavailable)?
    {
        size = size.saturating_add(chunk.len() as u64);
        if size > MAX_ARCHIVE {
            return Err(ReleaseError::Invalid("archive too large"));
        }
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        progress(size, total);
    }
    file.flush().await?;
    Ok((format!("{:x}", hasher.finalize()), size))
}

/// A pinned release (the caller chose the tag — used by versioned installs
/// and tests). Same sums contract as `fetch_latest`, without the redirect.
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
        SumsGet::NotModified => Err(ReleaseError::Invalid("etag without body")),
        SumsGet::Body { text, .. } => info_from_sums(desc, tag, &text, target),
    }
}
