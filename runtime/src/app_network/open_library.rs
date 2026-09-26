//! `shared/electron/book-metadata-open-library.ts` port — fixed-origin Open
//! Library edition + author lookup. JSON only, ≤256 KiB, ≤3 redirects that
//! must stay on the pinned origin; 404 → `None`.
use std::time::Duration;

use reqwest::Url;
use serde::Serialize;
use serde_json::Value;

use super::fetch::{fetch_https, FetchSpec, Fetched};
use super::AppNetworkCtx;

const OPEN_LIBRARY_ORIGIN: &str = "https://openlibrary.org";
const MAX_RESPONSE_BYTES: usize = 256 * 1024;
const MAX_AUTHORS: usize = 4;

/// `OpenLibraryBookMetadata` — normalized result the app renders as-is.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub(crate) struct OpenLibraryBookMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isbn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_date: Option<String>,
}

fn is_valid_isbn13(value: &str) -> bool {
    let digits: Vec<u32> = value.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() != 13 || value.len() != 13 {
        return false;
    }
    let sum: u32 = digits[..12]
        .iter()
        .enumerate()
        .map(|(i, d)| d * if i % 2 == 0 { 1 } else { 3 })
        .sum();
    (10 - (sum % 10)) % 10 == digits[12]
}

fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn string_list(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| text(Some(item)))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// `languageList` — `{key: "/languages/eng"}` objects → `eng, …`.
fn language_list(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("key").and_then(Value::as_str))
                .map(|key| key.rsplit('/').next().unwrap_or(""))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

async fn fetch_json(path: &str, ctx: &AppNetworkCtx) -> Result<Option<Value>, &'static str> {
    let origin = ctx
        .open_library_origin
        .as_deref()
        .unwrap_or(OPEN_LIBRARY_ORIGIN);
    let url = Url::parse(origin)
        .and_then(|base| base.join(path))
        .map_err(|_| "invalid-request")?;
    if url.origin().ascii_serialization() != origin.trim_end_matches('/') {
        return Err("invalid-request");
    }
    let spec = FetchSpec {
        accept: "application/json",
        accept_language: None,
        user_agent: "Kosmos Eden/0.5",
        max_bytes: MAX_RESPONSE_BYTES,
        allowed_content: None,
        same_origin_redirects: true,
        timeout: Duration::from_secs(10),
    };
    let fetched = fetch_https(url.as_str(), &spec, ctx).await;
    let Fetched {
        bytes,
        content_type,
        ..
    } = match fetched {
        Ok(fetched) => fetched,
        Err("not-found") => return Ok(None),
        Err(err) => return Err(err),
    };
    if !content_type.contains("application/json") {
        return Err("unavailable");
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "unavailable")?;
    if !value.is_object() {
        return Err("unavailable");
    }
    Ok(Some(value))
}

/// `authorNames` — `/authors/OL…A` keys only, capped at MAX_AUTHORS;
/// individual author failures degrade to missing names like the TS `.catch`.
async fn author_names(value: Option<&Value>, ctx: &AppNetworkCtx) -> String {
    let keys: Vec<String> = value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("key").and_then(Value::as_str))
                .filter(|key| {
                    let rest = key.strip_prefix("/authors/OL").unwrap_or("");
                    !rest.is_empty()
                        && rest.ends_with('A')
                        && rest[..rest.len() - 1].bytes().all(|b| b.is_ascii_digit())
                })
                .map(str::to_string)
                .take(MAX_AUTHORS)
                .collect()
        })
        .unwrap_or_default();
    let mut names = Vec::new();
    for key in keys {
        if let Ok(Some(author)) = fetch_json(&format!("{key}.json"), ctx).await {
            let name = text(author.get("name"));
            if !name.is_empty() {
                names.push(name);
            }
        }
    }
    names.join(", ")
}

/// `lookupOpenLibraryIsbn` — `bookMetadata.lookupIsbn` op body.
pub(crate) async fn lookup_isbn(
    isbn: &str,
    ctx: &AppNetworkCtx,
) -> Result<Option<OpenLibraryBookMetadata>, &'static str> {
    let isbn = isbn.trim();
    if !is_valid_isbn13(isbn) {
        return Err("invalid-request");
    }
    let Some(edition) = fetch_json(&format!("/isbn/{isbn}.json"), ctx).await? else {
        return Ok(None);
    };
    let author = author_names(edition.get("authors"), ctx).await;
    let cover_id = edition
        .get("covers")
        .and_then(Value::as_array)
        .and_then(|covers| covers.iter().find_map(|v| v.as_i64().filter(|n| *n > 0)));
    Ok(Some(OpenLibraryBookMetadata {
        title: Some(text(edition.get("title"))).filter(|s| !s.is_empty()),
        author: Some(author).filter(|s| !s.is_empty()),
        cover_image: cover_id
            .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg?default=false")),
        isbn: Some(isbn.to_string()),
        page_count: edition
            .get("number_of_pages")
            .and_then(Value::as_u64)
            .filter(|n| *n > 0),
        language: Some(language_list(edition.get("languages"))).filter(|s| !s.is_empty()),
        publisher: Some(string_list(edition.get("publishers"))).filter(|s| !s.is_empty()),
        published_date: Some(text(edition.get("publish_date"))).filter(|s| !s.is_empty()),
    }))
}
