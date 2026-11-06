use super::{FileIndexError, IndexedFile, Result, ScanOptions};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;

// Regression L2 (2026-05-24): bare "tmp", "cache", "out" были too greedy —
// блокировали legit user folders типа `D:\projects\my-app\tmp-output` или
// `D:\projects\cache-libs` без override. Оставлены только однозначные
// директории-артефакты сборки/окружения. "*.tmp"/"*.temp"/"**/Cache/**" из
// DEFAULT_IGNORE_PATTERNS продолжают защищать от системного шума.
const NOISY_FOLDER_NAMES: &[&str] = &[
    "$recycle.bin",
    ".cache",
    ".bun_cache",
    ".e2e",
    ".git",
    ".gradle",
    ".next",
    ".nuxt",
    ".parcel-cache",
    ".pnpm-store",
    ".tmp",
    ".turbo",
    ".venv",
    ".vite",
    "__pycache__",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "target",
    "venv",
];

// Regression M1 (2026-05-24): default patterns apply ALWAYS, separately from
// `exclude_noisy_folders`. Folder-name matches (.git, node_modules, target...)
// are handled by NOISY_FOLDER_NAMES via path_contains_noisy_folder and respect
// the toggle. Pattern-level entries here cover what name-matching can't: file
// extensions and absolute path families like AppData that you almost never
// want to index even in "power user" mode. AppData in particular: without it
// indexing %USERPROFILE% turns into 100k+ files of Electron/Chrome cache.
const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    "*.tmp",
    "*.temp",
    "**/AppData/**",
    "**/[Cc]ache/**",
    "**/[Cc]aches/**",
];

pub(super) fn filter_indexed_files(
    files: Vec<IndexedFile>,
    opts: &ScanOptions,
) -> Vec<IndexedFile> {
    let matcher = ignore_matcher(opts);
    files
        .into_iter()
        .filter(|file| should_index_path(Path::new(&file.path), opts, matcher.as_ref()))
        .collect()
}

#[cfg(windows)]
pub(super) fn is_drive_root(root: &Path) -> bool {
    // Regression H2 (2026-05-24): native folder picker returns "D:" or "D:\\"
    // — bare byte-length check missed both, so NTFS fast path silently never
    // engaged despite ntfs_accelerated=ON.
    let raw = root.to_string_lossy();
    let trimmed = raw.trim_end_matches(['\\', '/']);
    let bytes = trimmed.as_bytes();
    bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

pub(super) fn should_enter_dir(
    path: &Path,
    root: &Path,
    opts: &ScanOptions,
    matcher: Option<&GlobSet>,
) -> bool {
    if path == root {
        return true;
    }
    let relative = path.strip_prefix(root).unwrap_or(path);
    if opts.exclude_noisy_folders && is_noisy_folder(relative) {
        return false;
    }
    if !opts.include_hidden && is_dot_hidden(relative) {
        return false;
    }
    !matches_ignore_pattern(relative, matcher)
}

fn is_noisy_folder(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    NOISY_FOLDER_NAMES
        .iter()
        .any(|candidate| name == *candidate)
}

pub fn path_contains_noisy_folder(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy().to_lowercase();
        NOISY_FOLDER_NAMES
            .iter()
            .any(|candidate| name == *candidate)
    })
}

pub fn should_index_path(path: &Path, opts: &ScanOptions, matcher: Option<&GlobSet>) -> bool {
    if opts.exclude_noisy_folders && path_contains_noisy_folder(path) {
        return false;
    }
    if !opts.include_hidden && is_dot_hidden(path) {
        return false;
    }
    // Regression C2 (2026-05-24): on Windows "hidden" is the NTFS
    // FILE_ATTRIBUTE_HIDDEN bit, not a dot-prefix. Without this check the
    // NTFS fast scan + watcher events index AppData/ProgramData/etc despite
    // include_hidden=false.
    #[cfg(windows)]
    if !opts.include_hidden && path_has_hidden_attribute(path) {
        return false;
    }
    !matches_ignore_pattern(path, matcher)
}

#[cfg(windows)]
pub(super) fn path_has_hidden_attribute(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    // FILE_ATTRIBUTE_HIDDEN = 0x2, FILE_ATTRIBUTE_SYSTEM = 0x4 — system files
    // are typically also hidden in Explorer; treat them as hidden for indexing.
    const HIDDEN_OR_SYSTEM: u32 = 0x2 | 0x4;
    path.metadata()
        .ok()
        .map(|meta| meta.file_attributes() & HIDDEN_OR_SYSTEM != 0)
        .unwrap_or(false)
}

pub fn should_index_with_options(path: &Path, opts: &ScanOptions) -> bool {
    let matcher = ignore_matcher(opts);
    should_index_path(path, opts, matcher.as_ref())
}

pub(super) fn should_index_path_for_root(
    path: &Path,
    root: &Path,
    opts: &ScanOptions,
    matcher: Option<&GlobSet>,
) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    if opts.exclude_noisy_folders && path_contains_noisy_folder(relative) {
        return false;
    }
    if !opts.include_hidden && is_dot_hidden(relative) {
        return false;
    }
    !matches_ignore_pattern(relative, matcher)
}

pub fn validate_ignore_pattern(pattern: &str) -> Result<()> {
    let normalized = pattern.trim();
    if normalized.is_empty() {
        return Err(FileIndexError::InvalidSetting(
            "ignore pattern must not be empty".to_string(),
        ));
    }
    // Regression M10 (2026-05-24): Glob::new parses some patterns that later
    // fail at GlobSetBuilder.add → silently dropped at scan time.
    let mut builder = GlobSetBuilder::new();
    let glob = Glob::new(normalized)
        .map_err(|e| FileIndexError::InvalidSetting(format!("invalid ignore pattern: {e}")))?;
    builder.add(glob);
    builder
        .build()
        .map(|_| ())
        .map_err(|e| FileIndexError::InvalidSetting(format!("invalid ignore pattern: {e}")))
}

pub(super) fn ignore_matcher(opts: &ScanOptions) -> Option<GlobSet> {
    // Regression M1 (2026-05-24): default patterns (`**/AppData/**`,
    // `**/target/**`...) used to be gated by `exclude_noisy_folders`. When the
    // user turned that toggle off they lost ALL default ignores including the
    // critical AppData filter — `%USERPROFILE%` indexing then exploded. The
    // toggle now controls only the NOISY_FOLDER_NAMES check (component-level
    // names like "node_modules"); pattern-based defaults always apply.
    let mut builder = GlobSetBuilder::new();
    let mut added = false;
    for pattern in DEFAULT_IGNORE_PATTERNS
        .iter()
        .copied()
        .chain(opts.ignore_patterns.iter().map(String::as_str))
    {
        if add_glob_variants(&mut builder, pattern).is_ok() {
            added = true;
        }
    }
    if !added {
        return None;
    }
    builder.build().ok()
}

fn add_glob_variants(builder: &mut GlobSetBuilder, pattern: &str) -> std::result::Result<(), ()> {
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

fn matches_ignore_pattern(path: &Path, matcher: Option<&GlobSet>) -> bool {
    matcher.is_some_and(|matcher| {
        matcher.is_match(path)
            || path
                .file_name()
                .is_some_and(|name| matcher.is_match(Path::new(name)))
    })
}

fn is_dot_hidden(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name.starts_with('.') && name != "." && name != ".."
    })
}
