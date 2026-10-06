//! Shared DNS-pinned HTTPS GET (`shared/electron/book-metadata-fetch.ts` and
//! `remote-image-color.ts` ports): validate the URL, resolve the host, require
//! every resolved address to be public, pin the first address on the reqwest
//! client, follow ≤3 redirects with a fresh validation each hop, cap the body.
use std::net::SocketAddr;
use std::time::Duration;

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, LOCATION, USER_AGENT};
use reqwest::Url;

use super::{is_public_network_address, AppNetworkCtx};

pub(crate) const MAX_REDIRECTS: usize = 3;

pub(crate) struct FetchSpec<'a> {
    pub accept: &'a str,
    pub accept_language: Option<&'a str>,
    pub user_agent: &'a str,
    pub max_bytes: usize,
    /// `None` accepts any content type; otherwise an exact prefix match on the
    /// header value before `;`.
    pub allowed_content: Option<&'a [&'a str]>,
    /// `book-metadata-open-library.ts` keeps redirects on the fixed origin.
    pub same_origin_redirects: bool,
    pub timeout: Duration,
}

pub(crate) struct Fetched {
    pub final_url: Url,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// `parseBookMetadataHttpsUrl` — public HTTPS only, no credentials.
fn validate_url(raw: &str, ctx: &AppNetworkCtx) -> Result<Url, &'static str> {
    let url = Url::parse(raw.trim()).map_err(|_| "invalid-request")?;
    let plain_http = url.scheme() == "http" && ctx.allow_private_http;
    if (url.scheme() != "https" && !plain_http)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("forbidden");
    }
    if url.host_str().is_none() {
        return Err("invalid-request");
    }
    Ok(url)
}

async fn public_pinned_addr(url: &Url, ctx: &AppNetworkCtx) -> Result<SocketAddr, &'static str> {
    let host = url.host_str().ok_or("invalid-request")?;
    let port = url.port_or_known_default().ok_or("invalid-request")?;
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| "unavailable")?
        .collect();
    if addrs.is_empty() {
        return Err("unavailable");
    }
    if !ctx.allow_private_http
        && addrs
            .iter()
            .any(|addr| !is_public_network_address(&addr.ip().to_string()))
    {
        return Err("forbidden");
    }
    Ok(addrs[0])
}

async fn request_once(
    url: &Url,
    addr: SocketAddr,
    spec: &FetchSpec<'_>,
) -> Result<reqwest::Response, &'static str> {
    let host = url.host_str().ok_or("invalid-request")?;
    let client = engine_base::http::client_builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(spec.timeout)
        .connect_timeout(Duration::from_secs(10))
        .resolve(host, addr)
        .build()
        .map_err(|_| "unavailable")?;
    let mut request = client
        .get(url.clone())
        .header(ACCEPT, spec.accept)
        .header(USER_AGENT, spec.user_agent);
    if let Some(language) = spec.accept_language {
        request = request.header(ACCEPT_LANGUAGE, language);
    }
    request.send().await.map_err(|error| {
        if error.is_timeout() {
            "timeout"
        } else {
            "unavailable"
        }
    })
}

/// `fetchBookMetadataPage`/`dominantRemoteImageColor` fetch loop.
pub(crate) async fn fetch_https(
    raw_url: &str,
    spec: &FetchSpec<'_>,
    ctx: &AppNetworkCtx,
) -> Result<Fetched, &'static str> {
    let mut url = validate_url(raw_url, ctx)?;
    for redirect_count in 0..=MAX_REDIRECTS {
        let addr = public_pinned_addr(&url, ctx).await?;
        let response = request_once(&url, addr, spec).await?;
        let status = response.status();
        if status.is_redirection() {
            if redirect_count == MAX_REDIRECTS {
                return Err("unavailable");
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or("unavailable")?;
            let next = url.join(location).map_err(|_| "unavailable")?;
            if spec.same_origin_redirects && next.origin() != url.origin() {
                return Err("forbidden");
            }
            url = validate_url(next.as_str(), ctx)?;
            continue;
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err("not-found");
        }
        if !status.is_success() {
            return Err("unavailable");
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .map(|v| v.trim().to_lowercase())
            .unwrap_or_default();
        if let Some(allowed) = spec.allowed_content {
            if !allowed.iter().any(|t| content_type == *t) {
                return Err("unavailable");
            }
        }
        if response
            .content_length()
            .is_some_and(|n| n > spec.max_bytes as u64)
        {
            return Err("unavailable");
        }
        let mut bytes = Vec::new();
        let mut stream = response;
        while let Some(chunk) = stream.chunk().await.map_err(|_| "unavailable")? {
            if bytes.len() + chunk.len() > spec.max_bytes {
                return Err("unavailable");
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok(Fetched {
            final_url: url,
            content_type,
            bytes,
        });
    }
    Err("unavailable")
}
