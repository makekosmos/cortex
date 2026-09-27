//! `shared/electron/book-metadata-fetch.ts` port — public HTTPS page fetch
//! for the book-metadata import modal. The Electron browser-window fallback
//! (`book-metadata-browser.ts`) has no Engine equivalent: a detected browser
//! challenge returns `unavailable` so the app can surface a clear error.
use std::time::Duration;

use super::fetch::{fetch_https, FetchSpec};
use super::AppNetworkCtx;

const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";

#[derive(Debug, PartialEq)]
pub(crate) struct BookMetadataPage {
    pub final_url: String,
    pub html: String,
}

/// `looksLikeBookMetadataBrowserChallenge`.
pub(crate) fn looks_like_browser_challenge(html: &str) -> bool {
    let probe = html[..html.len().min(100_000)].to_lowercase();
    regex::Regex::new(r"ddos-guard|checking your browser|just a moment|провер(?:ка|яем) браузера")
        .map(|re| re.is_match(&probe))
        .unwrap_or(false)
}

/// `fetchBookMetadataPage` — DNS-pinned, redirect-bounded, HTML-only, 2 MiB
/// cap. The challenge probe runs on the fetched body; a challenge page is a
/// dead end without a real browser (`unavailable`).
pub(crate) async fn fetch_page(
    source: &str,
    ctx: &AppNetworkCtx,
) -> Result<BookMetadataPage, &'static str> {
    let spec = FetchSpec {
        accept: "text/html,application/xhtml+xml",
        accept_language: Some("ru-RU,ru;q=0.9,en;q=0.7"),
        user_agent: BROWSER_USER_AGENT,
        max_bytes: MAX_HTML_BYTES,
        allowed_content: Some(&["text/html", "application/xhtml+xml"]),
        same_origin_redirects: false,
        timeout: Duration::from_secs(15),
    };
    let fetched = fetch_https(source, &spec, ctx).await?;
    let html = String::from_utf8_lossy(&fetched.bytes).into_owned();
    if looks_like_browser_challenge(&html) {
        return Err("unavailable");
    }
    Ok(BookMetadataPage {
        final_url: fetched.final_url.to_string(),
        html,
    })
}
