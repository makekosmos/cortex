/// The Engine-side host shim: served at a reserved asset-relative name and
/// injected into every package HTML page, it bootstraps the launch-scoped
/// credentials and exposes the `window.kosmosApp` API the Electron host's
/// preload used to define. Kept as a real file so it reads and diffs as JS.
const HOST_SHIM: &str = include_str!("../host_shim.js");
/// Reserved asset name under each launch's asset token — `.kspkg` archives
/// cannot collide: the asset path is looked up after grant validation and
/// this name is answered by the Engine before the package store is read.
const HOST_SHIM_ASSET: &str = "__kosmos_host_shim.js";

async fn serve_asset(
    path: &str,
    package_service: Arc<PackageService>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
) -> HttpResponse {
    let Some(rest) = path.strip_prefix("/v1/apps/assets/") else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let Some((token, raw_asset)) = rest.split_once('/') else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    if token.is_empty()
        || token.len() > 128
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return asset_error(StatusCode::NOT_FOUND);
    }
    let Some(asset) = percent_decode(raw_asset) else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let grant = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .asset(token);
    let Some(grant) = grant else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    if asset == HOST_SHIM_ASSET {
        return asset_response(&asset, HOST_SHIM.as_bytes().to_vec());
    }
    let bytes = match package_service.read_app_asset(&grant.id, &grant.version, &grant.hash, &asset)
    {
        Ok(bytes) => bytes,
        Err(_) => return asset_error(StatusCode::NOT_FOUND),
    };
    asset_response(&asset, inject_host_shim(&asset, &bytes))
}

/// Inject the shim `<script>` tag as early as the served HTML allows — the
/// parser loads it synchronously, so the `kosmosApp` global exists before
/// the app's own scripts run; the async credential exchange is queued by
/// the shim itself.
fn inject_host_shim(asset: &str, bytes: &[u8]) -> Vec<u8> {
    let html = asset.rsplit_once('.').is_some_and(|(_, ext)| {
        ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm")
    });
    if !html {
        return bytes.to_vec();
    }
    let tag = format!("<script src=\"{HOST_SHIM_ASSET}\"></script>");
    // Search the raw bytes — `from_utf8_lossy` rewrites invalid sequences
    // to U+FFFD, shifting every later offset, so string offsets cannot be
    // reused on the original byte buffer.
    let Some(head) = head_tag_end(bytes) else {
        return [tag.as_bytes(), bytes].concat();
    };
    let mut out = Vec::with_capacity(bytes.len() + tag.len());
    out.extend_from_slice(&bytes[..head]);
    out.extend_from_slice(tag.as_bytes());
    out.extend_from_slice(&bytes[head..]);
    out
}

/// Offset just past a `<head>` open tag, byte-exact and ASCII
/// case-insensitive. The char after `head` must be `>`, `/` or whitespace —
/// that is what keeps `<header>` from matching.
fn head_tag_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(5).enumerate().find_map(|(at, window)| {
        if !window.eq_ignore_ascii_case(b"<head") {
            return None;
        }
        let next = bytes.get(at + 5)?;
        if !(matches!(next, b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n' | b'\x0c')) {
            return None;
        }
        bytes[at + 5..]
            .iter()
            .position(|b| *b == b'>')
            .map(|end| at + 5 + end + 1)
    })
}

fn asset_response(asset: &str, bytes: Vec<u8>) -> HttpResponse {
    let html = asset.rsplit_once('.').is_some_and(|(_, ext)| {
        ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm")
    });
    let content_type = mime_type(asset);
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header("cache-control", "no-store")
        .header("referrer-policy", "no-referrer")
        .header("x-content-type-options", "nosniff");
    if html {
        // connect-src 'self': the shim's bootstrap/ark/renew/revoke and the
        // SSE event stream are same-origin fetches.
        response = response.header(
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        );
    }
    response
        .body(boxed(Bytes::from(bytes)))
        .unwrap_or_else(|_| asset_error(StatusCode::INTERNAL_SERVER_ERROR))
}

fn asset_error(status: StatusCode) -> HttpResponse {
    Response::builder()
        .status(status)
        .header("cache-control", "no-store")
        .header("referrer-policy", "no-referrer")
        .body(boxed(Bytes::new()))
        .unwrap_or_else(|_| Response::new(boxed(Bytes::new())))
}

fn percent_decode(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let input = value.as_bytes();
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            if index + 2 >= input.len() {
                return None;
            }
            let high = (input[index + 1] as char).to_digit(16)? as u8;
            let low = (input[index + 2] as char).to_digit(16)? as u8;
            bytes.push((high << 4) | low);
            index += 3;
        } else {
            bytes.push(input[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).ok()
}

fn mime_type(path: &str) -> &'static str {
    match path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}