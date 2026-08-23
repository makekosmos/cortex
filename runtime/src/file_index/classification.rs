use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IndexedFileClass {
    TextLike,
    Media,
    Other,
}

pub(super) fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

pub(super) fn estimate_index_entry_size_bytes(path: &Path, name: &str) -> u64 {
    let path_bytes = path.as_os_str().to_string_lossy().len() as u64;
    let name_bytes = name.len() as u64;
    // files row + fts row + index overhead. This intentionally overestimates a
    // little so settings UI does not present unrealistically low storage costs.
    192u64
        .saturating_add(path_bytes.saturating_mul(2))
        .saturating_add(name_bytes.saturating_mul(2))
}

pub(super) fn classify_indexed_file(path: &Path) -> IndexedFileClass {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return IndexedFileClass::Other;
    };
    let ext = ext.to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "svg"
            | "heic"
            | "avif"
            | "mp4"
            | "mov"
            | "avi"
            | "mkv"
            | "webm"
            | "mp3"
            | "wav"
            | "flac"
            | "ogg"
            | "m4a"
    ) {
        return IndexedFileClass::Media;
    }
    if matches!(
        ext.as_str(),
        "txt"
            | "md"
            | "rs"
            | "toml"
            | "json"
            | "jsonc"
            | "yaml"
            | "yml"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "html"
            | "css"
            | "scss"
            | "less"
            | "xml"
            | "csv"
            | "tsv"
            | "sql"
            | "py"
            | "java"
            | "kt"
            | "kts"
            | "go"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "swift"
            | "sh"
            | "ps1"
            | "bat"
            | "cmd"
            | "ini"
            | "log"
    ) {
        return IndexedFileClass::TextLike;
    }
    IndexedFileClass::Other
}

pub(super) fn estimate_ignore_matcher(options: &ScanOptions) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut added = false;
    for pattern in ESTIMATE_DEFAULT_IGNORE_PATTERNS
        .iter()
        .copied()
        .chain(options.ignore_patterns.iter().map(String::as_str))
    {
        if estimate_add_glob_variants(&mut builder, pattern).is_ok() {
            added = true;
        }
    }
    if !added {
        return None;
    }
    builder.build().ok()
}

fn estimate_add_glob_variants(
    builder: &mut GlobSetBuilder,
    pattern: &str,
) -> std::result::Result<(), ()> {
    let normalized = pattern.trim();
    if normalized.is_empty() {
        return Ok(());
    }
    let glob = Glob::new(normalized).map_err(|_| ())?;
    builder.add(glob);
    if !normalized.contains('/') && !normalized.contains('\\') {
        let deep = Glob::new(&format!("**/{normalized}")).map_err(|_| ())?;
        builder.add(deep);
    }
    Ok(())
}

pub(super) fn estimate_should_index_path_for_root(
    path: &Path,
    root: &Path,
    options: &ScanOptions,
    matcher: Option<&GlobSet>,
) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if options.exclude_noisy_folders && scanner::path_contains_noisy_folder(relative) {
        return false;
    }
    if !options.include_hidden && estimate_is_dot_hidden(relative) {
        return false;
    }
    !estimate_matches_ignore_pattern(relative, matcher)
}

fn estimate_matches_ignore_pattern(path: &Path, matcher: Option<&GlobSet>) -> bool {
    matcher.is_some_and(|matcher| {
        matcher.is_match(path)
            || path
                .file_name()
                .is_some_and(|name| matcher.is_match(Path::new(name)))
    })
}

fn estimate_is_dot_hidden(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name.starts_with('.') && name != "." && name != ".."
    })
}
