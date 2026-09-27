//! `remote-image-color.ts` + the cover-drop storage path the Vue host
//! implements via `userData.writeBinary` — Engine fetches/stores/decodes so
//! the app only ever sees a local path and an optional `rgb(…)` string.
use std::path::PathBuf;
use std::time::Duration;

use sha2::Digest;

use super::color::{dominant_image_color, image_dimensions};
use super::fetch::{fetch_https, FetchSpec};
use super::AppNetworkCtx;

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_PIXELS: u64 = 40_000_000;
const ALLOWED_IMAGE_TYPES: &[&str] = &["image/jpeg", "image/png"];
const COVER_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

fn image_spec<'a>() -> FetchSpec<'a> {
    FetchSpec {
        accept: "image/jpeg,image/png",
        accept_language: None,
        user_agent: "Kosmos Eden/0.5",
        max_bytes: MAX_IMAGE_BYTES,
        allowed_content: Some(ALLOWED_IMAGE_TYPES),
        same_origin_redirects: false,
        timeout: Duration::from_secs(10),
    }
}

/// Decode + resize to 32×32 RGBA and compute `dominantImageColor` —
/// `nativeImage.createFromBuffer` + `resize` in the Electron version.
fn color_of(bytes: &[u8]) -> Option<String> {
    let (width, height) = image_dimensions(bytes)?;
    if width as u64 * height as u64 > MAX_PIXELS {
        return None;
    }
    let image = image::load_from_memory(bytes).ok()?;
    let resized = image.resize_exact(32, 32, image::imageops::FilterType::Triangle);
    let rgba = resized.to_rgba8();
    dominant_image_color(rgba.as_raw(), 4)
}

fn safe_app_id(app_id: &str) -> Result<&str, &'static str> {
    let ok = !app_id.is_empty()
        && app_id.len() <= 64
        && app_id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'))
        && app_id.split('.').count() >= 2;
    ok.then_some(app_id).ok_or("invalid-request")
}

fn data_dir_for(ctx: &AppNetworkCtx, app_id: &str, kind: &str) -> Result<PathBuf, &'static str> {
    let dir = ctx
        .data_dir
        .join("extension-data")
        .join(safe_app_id(app_id)?)
        .join(kind);
    std::fs::create_dir_all(&dir).map_err(|_| "unavailable")?;
    Ok(dir)
}

/// Persist bytes under `extension-data/<app>/<kind>/<name>` — returns the
/// absolute path for the app to hand to its image loader.
fn store_bytes(
    ctx: &AppNetworkCtx,
    app_id: &str,
    kind: &str,
    name: &str,
    bytes: &[u8],
) -> Result<String, &'static str> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("unavailable");
    }
    let path = data_dir_for(ctx, app_id, kind)?.join(name);
    std::fs::write(&path, bytes).map_err(|_| "unavailable")?;
    Ok(path.to_string_lossy().into_owned())
}

#[derive(Debug, PartialEq)]
pub(crate) struct FetchedImage {
    pub path: String,
    pub color: Option<String>,
}

/// `images.fetch` — remote cover/picture for display without app network:
/// pinned HTTPS fetch → image bytes → `remote-images/<sha256(url)>.<ext>`
/// → `{path, color}`.
pub(crate) async fn fetch_remote(
    url: &str,
    app_id: &str,
    ctx: &AppNetworkCtx,
) -> Result<FetchedImage, &'static str> {
    let fetched = fetch_https(url, &image_spec(), ctx).await?;
    if image_dimensions(&fetched.bytes).is_none_or(|(w, h)| w as u64 * h as u64 > MAX_PIXELS) {
        return Err("unavailable");
    }
    let ext = match fetched.content_type.as_str() {
        "image/png" => "png",
        _ => "jpg",
    };
    let name = format!("{:x}.{}", sha2::Sha256::digest(url.as_bytes()), ext);
    let path = store_bytes(ctx, app_id, "remote-images", &name, &fetched.bytes)?;
    let color = color_of(&fetched.bytes);
    Ok(FetchedImage { path, color })
}

/// `images.dominantColor` — `dominantRemoteImageColor` for `https:` sources;
/// a local absolute path reads through the Engine (≤10 MiB, image only).
pub(crate) async fn dominant_color(
    src: &str,
    ctx: &AppNetworkCtx,
) -> Result<Option<String>, &'static str> {
    let src = src.trim();
    if src.starts_with("https://") || (ctx.allow_private_http && src.starts_with("http://")) {
        let fetched = fetch_https(src, &image_spec(), ctx).await?;
        return Ok(color_of(&fetched.bytes));
    }
    let bytes = tokio::task::spawn_blocking({
        let src = src.to_string();
        move || {
            let meta = std::fs::metadata(&src).ok()?;
            if meta.len() > MAX_IMAGE_BYTES as u64 {
                return None;
            }
            std::fs::read(&src).ok()
        }
    })
    .await
    .map_err(|_| "unavailable")?;
    Ok(bytes.and_then(|bytes| color_of(&bytes)))
}

/// `images.storeCover` — the `saveCoverFile` storage half: the Engine reads
/// the dropped source file (size/extension checked like `file.type`/`size`),
/// writes `extension-data/<app>/book-covers/<entry>-<uuid>.<ext>` and returns
/// the stored path the header prop persists.
pub(crate) async fn store_cover(
    source_path: &str,
    entry_id: &str,
    app_id: &str,
    ctx: &AppNetworkCtx,
) -> Result<String, &'static str> {
    let source = PathBuf::from(source_path.trim());
    let ext = source
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .filter(|ext| COVER_EXTENSIONS.contains(&ext.as_str()))
        .ok_or("invalid-request")?;
    let entry = entry_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(64)
        .collect::<String>();
    let entry = if entry.is_empty() {
        "cover".to_string()
    } else {
        entry
    };
    // `file.size` gate parity: reject oversized sources by metadata before
    // reading so an app cannot make the Engine slurp an unbounded file into
    // memory (`store_bytes` only sees the already-read bytes otherwise).
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, &'static str> {
        let meta = std::fs::metadata(&source).map_err(|_| "not-found")?;
        if meta.len() > MAX_IMAGE_BYTES as u64 {
            return Err("unavailable");
        }
        std::fs::read(&source).map_err(|_| "not-found")
    })
    .await
    .map_err(|_| "unavailable")??;
    let name = format!("{entry}-{}.{}", uuid::Uuid::new_v4(), ext);
    store_bytes(ctx, app_id, "book-covers", &name, &bytes)
}
