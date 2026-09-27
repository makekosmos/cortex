//! Path-safety helpers — port of `safeVaultOutputPath`,
//! `resolveMarkdownVaultSourcePath`, and `safeMarkdownDefaultName` from
//! `shared/electron/markdown-vault.ts`. Component validation defers to the
//! `handle_relative_fs` rules (reject `.`/`..`/empty/separators/NUL); symlink
//! components are rejected at open time by that layer.

use crate::handle_relative_fs;
use std::io;
use std::path::PathBuf;

/// `safeVaultOutputPath` — validate an app-supplied relative path against the
/// TS contract: normalized to `/`, rejects absolute/drive/traversal, then each
/// component goes through the handle-relative rules (no `.`/`..`/separators).
pub fn parse_relative_path(relative_path: &str) -> io::Result<Vec<String>> {
    let normalized = relative_path.replace('\\', "/");
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "invalid-request");
    let bytes = normalized.as_bytes();
    let drive_prefixed = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if normalized.starts_with('/')
        || normalized == ".."
        || normalized.contains("../")
        || drive_prefixed
    {
        return Err(invalid());
    }
    let components: Vec<String> = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    if components.is_empty() {
        return Err(invalid());
    }
    let borrowed: Vec<&str> = components.iter().map(String::as_str).collect();
    handle_relative_fs::validate_components(&borrowed)?;
    Ok(components)
}

/// `resolveMarkdownVaultSourcePath` — the only shapes that may name a local
/// copy source: `file:` URLs, `kosmos-local-image://file/…` URLs, and plain
/// absolute paths. Everything else (relative, empty, remote schemes) is None.
pub fn resolve_vault_source_path(source: &str) -> Option<PathBuf> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return None;
    }
    // `get(..5)` — `trimmed[..5]` panics when byte 5 splits a multi-byte char
    // (`"文件:…"`), and a non-ASCII prefix cannot equal `file:` anyway.
    if trimmed
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    {
        return file_url_to_path(trimmed).filter(|p| p.is_absolute());
    }
    if let Some(decoded) = super::local_image::parse_local_image_request_url(trimmed) {
        let path = PathBuf::from(decoded);
        return path.is_absolute().then_some(path);
    }
    let path = PathBuf::from(trimmed);
    path.is_absolute().then_some(path)
}

/// `fileURLToPath` subset: `file://` + optional host (empty/`localhost`),
/// percent-decoded path, `/C:/…` drive-letter normalization for Windows.
fn file_url_to_path(raw: &str) -> Option<PathBuf> {
    let rest = raw.get(5..)?;
    let rest = rest.strip_prefix("//").unwrap_or(rest);
    let (host, path) = match rest.split_once('/') {
        Some((host, path)) => (host, format!("/{path}")),
        None => (rest, String::new()),
    };
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return None;
    }
    if path.is_empty() {
        return None;
    }
    let decoded = super::local_image::percent_decode(&path)?;
    // `/C:/…` → `C:/…` on Windows-style URLs (kept literal on unix paths too —
    // the absolute check below decides acceptance).
    #[cfg(windows)]
    let decoded = {
        let b = decoded.as_bytes();
        if b.len() >= 4
            && b[0] == b'/'
            && b[1].is_ascii_alphabetic()
            && b[2] == b':'
            && b[3] == b'/'
        {
            decoded[1..].to_string()
        } else {
            decoded
        }
    };
    Some(PathBuf::from(decoded))
}

/// `safeMarkdownDefaultName` — sanitize a user-supplied default file name.
pub fn safe_markdown_default_name(name: Option<&str>) -> String {
    let fallback = "eden-object.md";
    let Some(name) = name else {
        return fallback.to_string();
    };
    let base = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || (c as u32) < 32
            {
                '-'
            } else {
                c
            }
        })
        .collect::<String>();
    let base = base.trim();
    if base.is_empty() {
        return fallback.to_string();
    }
    if base.to_lowercase().ends_with(".md") {
        base.to_string()
    } else {
        format!("{base}.md")
    }
}
