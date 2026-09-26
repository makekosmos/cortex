//! `kosmos-local-image://file/<percent-encoded>` URL helpers — port of
//! `shared/electron/local-image-protocol.ts` (URL/mime/parse only; the
//! markdown-brain search fallback is a display concern, not vault IO).

pub const LOCAL_IMAGE_PROTOCOL: &str = "kosmos-local-image";
pub(crate) const LOCAL_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "avif"];

pub fn local_image_url(path: &str) -> String {
    format!("{LOCAL_IMAGE_PROTOCOL}://file/{}", percent_encode(path))
}

/// `parseLocalImageRequestUrl` — returns the decoded file path when the URL
/// matches `kosmos-local-image://file/…` and ends in a supported extension.
pub fn parse_local_image_request_url(raw: &str) -> Option<String> {
    let rest = raw.strip_prefix(&format!("{LOCAL_IMAGE_PROTOCOL}://"))?;
    // URL semantics: `host/path`; the host must be exactly `file`.
    let (host, path) = rest.split_once('/')?;
    if host != "file" {
        return None;
    }
    let decoded = percent_decode(path.trim_start_matches('/'))?;
    if decoded.is_empty() || !is_supported_local_image_path(&decoded) {
        return None;
    }
    Some(decoded)
}

pub fn is_supported_local_image_path(path: &str) -> bool {
    extension_of(path).is_some_and(|ext| LOCAL_IMAGE_EXTENSIONS.contains(&ext.as_str()))
}

pub fn local_image_mime_type(path: &str) -> &'static str {
    match extension_of(path).as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        _ => "application/octet-stream",
    }
}

fn extension_of(path: &str) -> Option<String> {
    let name = path.rsplit(['/', '\\']).next()?;
    let (stem, ext) = name.rsplit_once('.')?;
    // `path.extname` parity: a leading-dot name (".png") has no extension.
    if stem.is_empty() {
        return None;
    }
    let ext = ext.to_ascii_lowercase();
    (!ext.is_empty()).then_some(ext)
}

pub fn percent_decode(value: &str) -> Option<String> {
    let input = value.as_bytes();
    let mut bytes = Vec::with_capacity(input.len());
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

fn percent_encode(value: &str) -> String {
    // encodeURIComponent semantics for path bodies.
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let safe = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            );
        if safe {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}
