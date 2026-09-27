//! Async op-level tests against a local fake HTTP server — the Open Library
//! golden path from `book-metadata-open-library.test.ts` plus fetch limits,
//! redirects, cover storage and remote image color.
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::Response;
use serde_json::json;

use super::tests::{ctx, json_response, redirect, spawn_server};
use super::*;

#[tokio::test]
async fn lookup_isbn_maps_edition_and_resolves_authors() {
    let origin = spawn_server(Arc::new(|path: &str| {
        if path == "/isbn/9780140328721.json" {
            return redirect("/books/OL7353617M.json");
        }
        if path == "/authors/OL34184A.json" {
            return json_response(json!({ "name": "Roald Dahl" }));
        }
        json_response(json!({
            "title": "Fantastic Mr. Fox",
            "authors": [{ "key": "/authors/OL34184A" }],
            "publishers": ["Puffin"],
            "publish_date": "October 1, 1988",
            "number_of_pages": 96,
            "languages": [{ "key": "/languages/eng" }],
            "covers": [15152634],
        }))
    }))
    .await;
    let ctx = ctx(std::env::temp_dir(), Some(origin));
    let metadata = open_library::lookup_isbn("9780140328721", &ctx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(&metadata).unwrap(),
        json!({
            "title": "Fantastic Mr. Fox",
            "author": "Roald Dahl",
            "cover_image": "https://covers.openlibrary.org/b/id/15152634-L.jpg?default=false",
            "isbn": "9780140328721",
            "page_count": 96,
            "language": "eng",
            "publisher": "Puffin",
            "published_date": "October 1, 1988",
        })
    );
}

#[tokio::test]
async fn lookup_isbn_rejects_external_redirects_and_invalid_isbn() {
    let origin = spawn_server(Arc::new(|_| redirect("https://example.com/book.json"))).await;
    let ctx = ctx(std::env::temp_dir(), Some(origin));
    assert_eq!(
        open_library::lookup_isbn("9780140328721", &ctx).await,
        Err("forbidden")
    );
    assert_eq!(
        open_library::lookup_isbn("not-an-isbn", &ctx).await,
        Err("invalid-request")
    );
}

#[tokio::test]
async fn lookup_isbn_returns_none_for_missing_edition() {
    let origin = spawn_server(Arc::new(|_| {
        Response::builder()
            .status(404)
            .body(Full::new(Bytes::new()))
            .unwrap()
    }))
    .await;
    let ctx = ctx(std::env::temp_dir(), Some(origin));
    assert_eq!(
        open_library::lookup_isbn("9780140328721", &ctx).await,
        Ok(None)
    );
}

#[tokio::test]
async fn fetch_page_caps_body_and_rejects_non_html() {
    let origin = spawn_server(Arc::new(|path: &str| {
        if path == "/huge" {
            return Response::builder()
                .header("content-type", "text/html")
                .header("content-length", (2 * 1024 * 1024 + 1).to_string())
                .body(Full::new(Bytes::from("<h1>x</h1>")))
                .unwrap();
        }
        if path == "/json" {
            return json_response(json!({}));
        }
        Response::builder()
            .header("content-type", "text/html; charset=utf-8")
            .body(Full::new(Bytes::from("<h1>Книга</h1>")))
            .unwrap()
    }))
    .await;
    let ctx = ctx(std::env::temp_dir(), None);
    let page = browser_page::fetch_page(&format!("{origin}/book"), &ctx)
        .await
        .unwrap();
    assert_eq!(page.html, "<h1>Книга</h1>");
    assert!(page.final_url.ends_with("/book"));
    assert_eq!(
        browser_page::fetch_page(&format!("{origin}/huge"), &ctx).await,
        Err("unavailable")
    );
    assert_eq!(
        browser_page::fetch_page(&format!("{origin}/json"), &ctx).await,
        Err("unavailable")
    );
}

#[tokio::test]
async fn fetch_page_limits_redirects_and_blocks_plain_http_in_prod() {
    let origin = spawn_server(Arc::new(|path: &str| {
        let next = path.trim_start_matches('/').parse::<u32>().unwrap_or(0) + 1;
        redirect(&format!("http://localhost/{next}"))
    }))
    .await;
    let test_ctx = ctx(std::env::temp_dir(), None);
    assert_eq!(
        browser_page::fetch_page(&format!("{origin}/0"), &test_ctx).await,
        Err("unavailable")
    );
    let prod_ctx = AppNetworkCtx {
        data_dir: std::env::temp_dir(),
        allow_private_http: false,
        open_library_origin: None,
    };
    assert_eq!(
        browser_page::fetch_page(&format!("{origin}/book"), &prod_ctx).await,
        Err("forbidden")
    );
}

#[tokio::test]
async fn fetch_page_flags_browser_challenges() {
    let origin = spawn_server(Arc::new(|_| {
        Response::builder()
            .header("content-type", "text/html")
            .body(Full::new(Bytes::from(
                "<title>Checking your browser</title>",
            )))
            .unwrap()
    }))
    .await;
    let ctx = ctx(std::env::temp_dir(), None);
    assert_eq!(
        browser_page::fetch_page(&format!("{origin}/book"), &ctx).await,
        Err("unavailable")
    );
}

#[tokio::test]
async fn store_cover_copies_dropped_file_into_app_data() {
    let dir = std::env::temp_dir().join(format!("app-net-{}", uuid::Uuid::new_v4()));
    let source_dir = dir.join("inbox");
    std::fs::create_dir_all(&source_dir).unwrap();
    let png = {
        let image = image::RgbaImage::from_pixel(8, 8, image::Rgba([136, 86, 41, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    };
    let source = source_dir.join("cover.png");
    std::fs::write(&source, &png).unwrap();
    let ctx = ctx(dir.clone(), None);
    let stored =
        image_ops::store_cover(source.to_str().unwrap(), "b-1", "com.kosmos.memoria", &ctx)
            .await
            .unwrap();
    assert!(stored.contains("book-covers"));
    assert!(std::path::Path::new(&stored).exists());
    assert!(!std::fs::read(&stored).unwrap().is_empty());
    let color = image_ops::dominant_color(&stored, &ctx).await.unwrap();
    assert_eq!(color.as_deref(), Some("rgb(136 86 41)"));

    let bad = source_dir.join("cover.exe");
    std::fs::write(&bad, b"MZ").unwrap();
    assert_eq!(
        image_ops::store_cover(bad.to_str().unwrap(), "b-1", "com.kosmos.memoria", &ctx).await,
        Err("invalid-request")
    );
    let missing = source_dir.join("absent.png");
    assert_eq!(
        image_ops::store_cover(missing.to_str().unwrap(), "b-1", "com.kosmos.memoria", &ctx).await,
        Err("not-found")
    );
    assert_eq!(
        image_ops::store_cover(source.to_str().unwrap(), "b-1", "not an id", &ctx).await,
        Err("invalid-request")
    );
}

#[test]
fn browser_challenge_probe_truncates_on_char_boundary() {
    // The 100_000-byte probe cap must land on a char boundary — a multi-byte
    // char straddling the cap used to panic `looks_like_browser_challenge`.
    let mut flagged = "just a moment".to_string();
    flagged.push_str(&"a".repeat(99_986));
    flagged.push('é'); // occupies bytes 99_999..=100_000
    flagged.push_str(&"b".repeat(50_000));
    assert!(browser_page::looks_like_browser_challenge(&flagged));
    let mut benign = "a".repeat(99_999);
    benign.push('é');
    benign.push_str(&"b".repeat(50_000));
    assert!(!browser_page::looks_like_browser_challenge(&benign));
}

#[tokio::test]
async fn store_cover_rejects_oversized_source_without_reading() {
    let dir = std::env::temp_dir().join(format!("app-net-{}", uuid::Uuid::new_v4()));
    let source_dir = dir.join("inbox");
    std::fs::create_dir_all(&source_dir).unwrap();
    let big = source_dir.join("big.png");
    std::fs::write(&big, vec![0u8; 10 * 1024 * 1024 + 1]).unwrap();
    let ctx = ctx(dir, None);
    assert_eq!(
        image_ops::store_cover(big.to_str().unwrap(), "b-1", "com.kosmos.memoria", &ctx).await,
        Err("unavailable")
    );
}

#[tokio::test]
async fn images_fetch_stores_remote_and_returns_color() {
    let png = {
        let image = image::RgbaImage::from_pixel(16, 16, image::Rgba([10, 130, 200, 255]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    };
    let origin = spawn_server(Arc::new(move |path: &str| {
        if path == "/cover.png" {
            return Response::builder()
                .header("content-type", "image/png")
                .body(Full::new(Bytes::from(png.clone())))
                .unwrap();
        }
        Response::builder()
            .header("content-type", "text/html")
            .body(Full::new(Bytes::from("<h1>nope</h1>")))
            .unwrap()
    }))
    .await;
    let dir = std::env::temp_dir().join(format!("app-net-{}", uuid::Uuid::new_v4()));
    let ctx = ctx(dir.clone(), None);
    let fetched =
        image_ops::fetch_remote(&format!("{origin}/cover.png"), "com.kosmos.memoria", &ctx)
            .await
            .unwrap();
    assert!(fetched.path.contains("remote-images"));
    assert!(fetched.path.ends_with(".png"));
    assert_eq!(fetched.color.as_deref(), Some("rgb(10 130 200)"));
    assert!(std::path::Path::new(&fetched.path).exists());
    assert_eq!(
        image_ops::fetch_remote(&format!("{origin}/page"), "com.kosmos.memoria", &ctx).await,
        Err("unavailable")
    );
    assert_eq!(
        image_ops::fetch_remote(&format!("{origin}/cover.png"), "bad", &ctx).await,
        Err("invalid-request")
    );
}

#[tokio::test]
async fn handle_validates_ops_and_params() {
    let ctx = ctx(std::env::temp_dir(), None);
    assert_eq!(
        handle("bookMetadata.evil", &json!({}), &ctx).await,
        Err("invalid-request")
    );
    assert_eq!(
        handle("bookMetadata.lookupIsbn", &json!({}), &ctx).await,
        Err("invalid-request")
    );
    assert_eq!(
        handle("bookMetadata.fetchPage", &json!({ "url": "ftp://x" }), &ctx).await,
        Err("forbidden")
    );
    assert_eq!(
        handle("images.dominantColor", &json!({ "src": "" }), &ctx).await,
        Err("invalid-request")
    );
}
