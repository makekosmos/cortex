use super::{FileIndexError, IndexedFile, NtfsStatus, Result, ScanOptions};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
mod ntfs;

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

#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub phase: String,
    pub root: Option<String>,
    pub roots_done: usize,
    pub roots_total: usize,
    pub files_seen: usize,
    pub files_indexed: usize,
    pub message: String,
}

pub fn scan_roots_with_progress(
    roots: &[PathBuf],
    opts: &ScanOptions,
    mut on_progress: impl FnMut(ScanProgress),
    mut on_ntfs: impl FnMut(NtfsStatus, Option<String>),
    is_cancelled: impl Fn() -> bool,
) -> Vec<IndexedFile> {
    let mut files = Vec::new();
    let roots_total = roots.len();
    let mut ntfs_any_drive = false;
    let mut ntfs_any_active = false;
    let mut ntfs_any_fallback = false;
    let mut ntfs_note: Option<String> = None;
    for (idx, root) in roots.iter().enumerate() {
        if is_cancelled() {
            break;
        }
        if !root.is_dir() {
            continue;
        }
        on_progress(ScanProgress {
            phase: "scanning".to_string(),
            root: Some(root.to_string_lossy().into_owned()),
            roots_done: idx,
            roots_total,
            files_seen: files.len(),
            files_indexed: files.len(),
            message: "Сканируем файлы".to_string(),
        });
        let (root_files, root_ntfs) = scan_root(
            root,
            opts,
            idx,
            roots_total,
            &mut on_progress,
            &is_cancelled,
        );
        files.extend(root_files);
        if is_cancelled() {
            break;
        }
        if let Some((status, note)) = root_ntfs {
            ntfs_any_drive = true;
            match status {
                NtfsStatus::Active => ntfs_any_active = true,
                NtfsStatus::Fallback | NtfsStatus::Unavailable => {
                    ntfs_any_fallback = true;
                    if ntfs_note.is_none() {
                        ntfs_note = note;
                    }
                }
                _ => {}
            }
        }
        on_progress(ScanProgress {
            phase: "scanning".to_string(),
            root: Some(root.to_string_lossy().into_owned()),
            roots_done: idx + 1,
            roots_total,
            files_seen: files.len(),
            files_indexed: files.len(),
            message: "Сканируем файлы".to_string(),
        });
    }
    if opts.ntfs_accelerated {
        let status = if !ntfs_any_drive {
            // ntfs_accelerated=true но нет drive roots → ничего пробовать
            NtfsStatus::Unknown
        } else if ntfs_any_active {
            NtfsStatus::Active
        } else if ntfs_any_fallback {
            NtfsStatus::Fallback
        } else {
            NtfsStatus::Unknown
        };
        on_ntfs(status, ntfs_note);
    } else {
        on_ntfs(NtfsStatus::Disabled, None);
    }
    files
}

fn scan_root(
    root: &Path,
    opts: &ScanOptions,
    root_index: usize,
    roots_total: usize,
    on_progress: &mut impl FnMut(ScanProgress),
    is_cancelled: &impl Fn() -> bool,
) -> (Vec<IndexedFile>, Option<(NtfsStatus, Option<String>)>) {
    #[cfg(windows)]
    if opts.ntfs_accelerated && is_drive_root(root) {
        // Regression H1 (2026-05-24): NTFS fast path does not walk .gitignore
        // files, so respect_gitignore would silently be ignored. When the user
        // wants gitignore semantics, fall back to user-mode walk.
        if opts.respect_gitignore {
            tracing::info!(
                target: "file_index",
                root = %root.to_string_lossy(),
                "ntfs fast scan skipped: respect_gitignore is on"
            );
            let files = scan_walk_root(
                root,
                opts,
                root_index,
                roots_total,
                on_progress,
                is_cancelled,
            );
            return (
                files,
                Some((
                    NtfsStatus::Fallback,
                    Some("Учитывается .gitignore — NTFS режим не активен".to_string()),
                )),
            );
        }
        on_progress(ScanProgress {
            phase: "ntfs".to_string(),
            root: Some(root.to_string_lossy().into_owned()),
            roots_done: root_index,
            roots_total,
            files_seen: 0,
            files_indexed: 0,
            message: "Быстрый NTFS scan".to_string(),
        });
        match ntfs::scan_drive_root(root, opts.exclude_noisy_folders) {
            Ok(files) => {
                let filtered = filter_indexed_files(files, opts);
                on_progress(ScanProgress {
                    phase: "filtering".to_string(),
                    root: Some(root.to_string_lossy().into_owned()),
                    roots_done: root_index,
                    roots_total,
                    files_seen: filtered.len(),
                    files_indexed: filtered.len(),
                    message: "Фильтруем результаты NTFS".to_string(),
                });
                tracing::info!(
                    target: "file_index",
                    root = %root.to_string_lossy(),
                    total = filtered.len(),
                    "ntfs fast scan finished"
                );
                return (filtered, Some((NtfsStatus::Active, None)));
            }
            Err(error) => {
                tracing::warn!(
                    target: "file_index",
                    root = %root.to_string_lossy(),
                    error = %error,
                    "ntfs fast scan unavailable; using walk fallback"
                );
                let files = scan_walk_root(
                    root,
                    opts,
                    root_index,
                    roots_total,
                    on_progress,
                    is_cancelled,
                );
                return (
                    files,
                    Some((NtfsStatus::Unavailable, Some(error.to_string()))),
                );
            }
        }
    }
    #[cfg(windows)]
    let drive_marker = if opts.ntfs_accelerated && is_drive_root(root) {
        Some((NtfsStatus::Fallback, None))
    } else {
        None
    };
    #[cfg(not(windows))]
    let drive_marker: Option<(NtfsStatus, Option<String>)> = None;
    (
        scan_walk_root(
            root,
            opts,
            root_index,
            roots_total,
            on_progress,
            is_cancelled,
        ),
        drive_marker,
    )
}

fn scan_walk_root(
    root: &Path,
    opts: &ScanOptions,
    root_index: usize,
    roots_total: usize,
    on_progress: &mut impl FnMut(ScanProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Vec<IndexedFile> {
    let mut builder = WalkBuilder::new(root);
    builder
        .follow_links(false)
        .hidden(!opts.include_hidden)
        .git_ignore(opts.respect_gitignore)
        .git_global(opts.respect_gitignore)
        .git_exclude(opts.respect_gitignore)
        .parents(opts.respect_gitignore)
        .add_custom_ignore_filename(".rayignore");
    let matcher = ignore_matcher(opts);
    let mut files = Vec::new();
    let mut seen = 0usize;
    for entry in builder.build().filter_map(|entry| entry.ok()) {
        if is_cancelled() {
            break;
        }
        let path = entry.path();
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
            && !should_enter_dir(path, root, opts, matcher.as_ref())
        {
            continue;
        }
        if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
            continue;
        }
        if !should_index_path_for_root(path, root, opts, matcher.as_ref()) {
            continue;
        }
        seen += 1;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.is_empty() {
            continue;
        }
        // Regression L7 (2026-05-24): unwrap_or_default() used to silently
        // bury metadata failures as epoch 0 (1970-01-01) — breaks future
        // "recently modified" sorts and hides bad permissions. Log at trace.
        let mtime = match entry.metadata() {
            Ok(meta) => meta
                .modified()
                .ok()
                .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|mtime| mtime.as_secs() as i64)
                .unwrap_or(0),
            Err(e) => {
                tracing::trace!(
                    target: "file_index",
                    path = %entry.path().to_string_lossy(),
                    error = %e,
                    "metadata read failed; using mtime=0"
                );
                0
            }
        };
        files.push(IndexedFile {
            path: entry.path().to_string_lossy().into_owned(),
            name,
            mtime,
        });
        if seen == 1 || seen.is_multiple_of(500) {
            on_progress(ScanProgress {
                phase: "scanning".to_string(),
                root: Some(root.to_string_lossy().into_owned()),
                roots_done: root_index,
                roots_total,
                files_seen: seen,
                files_indexed: files.len(),
                message: "Сканируем файлы".to_string(),
            });
        }
    }
    files
}

fn filter_indexed_files(files: Vec<IndexedFile>, opts: &ScanOptions) -> Vec<IndexedFile> {
    let matcher = ignore_matcher(opts);
    files
        .into_iter()
        .filter(|file| should_index_path(Path::new(&file.path), opts, matcher.as_ref()))
        .collect()
}

#[cfg(windows)]
fn is_drive_root(root: &Path) -> bool {
    // Regression H2 (2026-05-24): native folder picker returns "D:" or "D:\\"
    // — bare byte-length check missed both, so NTFS fast path silently never
    // engaged despite ntfs_accelerated=ON.
    let raw = root.to_string_lossy();
    let trimmed = raw.trim_end_matches(['\\', '/']);
    let bytes = trimmed.as_bytes();
    bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn should_enter_dir(
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
fn path_has_hidden_attribute(path: &Path) -> bool {
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

fn should_index_path_for_root(
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

fn ignore_matcher(opts: &ScanOptions) -> Option<GlobSet> {
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

pub fn default_roots() -> Vec<PathBuf> {
    if let Ok(roots) = std::env::var("KEPLER_FILE_INDEX_ROOTS") {
        return roots
            .split(';')
            .map(str::trim)
            .filter(|root| !root.is_empty())
            .map(PathBuf::from)
            .collect();
    }
    if std::env::var("KOSMOS_TEST_MODE").as_deref() == Ok("1") {
        return Vec::new();
    }
    // См. postmortems.md § 2026-06-08: broad profile scans are opt-in only.
    Vec::new()
}

#[cfg(windows)]
pub fn open_file(path: &str) -> Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "", path])
        .spawn()
        .map(|_| ())
        .map_err(|e| FileIndexError::Open(e.to_string()))
}

#[cfg(not(windows))]
pub fn open_file(path: &str) -> Result<()> {
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| FileIndexError::Open(e.to_string()))
}

#[cfg(test)]
mod default_roots_tests {
    use super::*;

    // Regression L1 (2026-05-24): cargo test parallelizes tests within a
    // single binary by default. Tests that mutate process env races with any
    // other test reading the same vars. This static lock serializes them.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn env_override_takes_precedence() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Сохраняем и восстанавливаем env чтобы не ломать другие тесты в
        // том же процессе.
        let prev_roots = std::env::var("KEPLER_FILE_INDEX_ROOTS").ok();
        let prev_test = std::env::var("KOSMOS_TEST_MODE").ok();
        // SAFETY: тесты в этом модуле выполняются последовательно (cargo test
        // не запускает несколько тестов одного бинаря параллельно по умолчанию
        // только если --test-threads=1; на больших проектах могут параллельно,
        // но здесь восстановление env идёт сразу).
        unsafe {
            std::env::set_var("KEPLER_FILE_INDEX_ROOTS", r"C:\;D:\projects");
            std::env::remove_var("KOSMOS_TEST_MODE");
        }
        let roots = default_roots();
        assert_eq!(
            roots,
            vec![PathBuf::from(r"C:\"), PathBuf::from(r"D:\projects")],
        );

        unsafe {
            match prev_roots {
                Some(v) => std::env::set_var("KEPLER_FILE_INDEX_ROOTS", v),
                None => std::env::remove_var("KEPLER_FILE_INDEX_ROOTS"),
            }
            match prev_test {
                Some(v) => std::env::set_var("KOSMOS_TEST_MODE", v),
                None => std::env::remove_var("KOSMOS_TEST_MODE"),
            }
        }
    }

    #[test]
    fn production_default_roots_are_empty_without_opt_in() {
        // Regression: 2026-06-08. Startup must not scan USERPROFILE unless the
        // user/process explicitly opts into roots through KEPLER_FILE_INDEX_ROOTS.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev_roots = std::env::var("KEPLER_FILE_INDEX_ROOTS").ok();
        let prev_test = std::env::var("KOSMOS_TEST_MODE").ok();
        let prev_profile = std::env::var("USERPROFILE").ok();
        unsafe {
            std::env::remove_var("KEPLER_FILE_INDEX_ROOTS");
            std::env::remove_var("KOSMOS_TEST_MODE");
            std::env::set_var("USERPROFILE", r"C:\Users\real-user");
        }

        let roots = default_roots();

        assert!(roots.is_empty());

        unsafe {
            match prev_roots {
                Some(v) => std::env::set_var("KEPLER_FILE_INDEX_ROOTS", v),
                None => std::env::remove_var("KEPLER_FILE_INDEX_ROOTS"),
            }
            match prev_test {
                Some(v) => std::env::set_var("KOSMOS_TEST_MODE", v),
                None => std::env::remove_var("KOSMOS_TEST_MODE"),
            }
            match prev_profile {
                Some(v) => std::env::set_var("USERPROFILE", v),
                None => std::env::remove_var("USERPROFILE"),
            }
        }
    }

    #[test]
    fn scan_walk_stops_when_cancelled() {
        // Regression: 2026-06-08. Generation checks after the full walk were too
        // late; scanner must observe cancellation inside the loop.
        let root = tempfile::tempdir().unwrap();
        for n in 0..1_000 {
            std::fs::write(root.path().join(format!("file-{n}.txt")), b"v").unwrap();
        }
        let opts = ScanOptions {
            exclude_noisy_folders: true,
            respect_gitignore: true,
            include_hidden: false,
            ntfs_accelerated: false,
            ignore_patterns: Vec::new(),
        };
        let seen = std::sync::atomic::AtomicUsize::new(0);
        let files = scan_roots_with_progress(
            &[root.path().to_path_buf()],
            &opts,
            |_| {},
            |_, _| {},
            || seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 8,
        );

        assert!(files.len() < 1_000);
    }

    #[test]
    fn ignore_pattern_matches_file_name_anywhere() {
        let opts = ScanOptions {
            exclude_noisy_folders: true,
            respect_gitignore: true,
            include_hidden: false,
            ntfs_accelerated: false,
            ignore_patterns: vec!["*.tmp".to_string()],
        };
        assert!(!should_index_with_options(
            Path::new(r"D:\docs\scratch.tmp"),
            &opts
        ));
        assert!(should_index_with_options(
            Path::new(r"D:\docs\notes.md"),
            &opts
        ));
    }

    #[cfg(windows)]
    #[test]
    fn drive_root_detection_accepts_picker_variants() {
        // Regression H2 (2026-05-24): native folder picker may return any of
        // "D:", "D:\\", "D:\\\\", "d:/" — NTFS fast path must engage for all
        // of them, not only the canonical "D:\\".
        assert!(is_drive_root(Path::new("D:")));
        assert!(is_drive_root(Path::new(r"D:\")));
        assert!(is_drive_root(Path::new(r"D:\\")));
        assert!(is_drive_root(Path::new("d:/")));
        assert!(!is_drive_root(Path::new(r"D:\Projects")));
        assert!(!is_drive_root(Path::new("notadrive")));
    }

    #[test]
    fn validate_ignore_pattern_rejects_unbuildable_globs() {
        // Regression M10 (2026-05-24): "[abc" parses through Glob::new but
        // fails at GlobSetBuilder.build → silent drop at scan time.
        assert!(validate_ignore_pattern("[abc").is_err());
        assert!(validate_ignore_pattern("*.tmp").is_ok());
        assert!(validate_ignore_pattern("   ").is_err());
    }

    #[test]
    fn hidden_dot_paths_are_excluded_by_default() {
        let opts = ScanOptions {
            exclude_noisy_folders: true,
            respect_gitignore: true,
            include_hidden: false,
            ntfs_accelerated: false,
            ignore_patterns: Vec::new(),
        };
        // Synthetic path — no I/O happens, only Path component inspection.
        // Forward slashes parse identically on Windows and POSIX (backslash
        // is a literal char on Linux, so a Windows-style path would collapse
        // to a single non-dot component on CI).
        assert!(!should_index_with_options(Path::new("docs/.secret"), &opts));
    }

    #[cfg(windows)]
    #[test]
    fn ntfs_hidden_attribute_detected_by_path_helper() {
        // Regression C2 (2026-05-24): on Windows "hidden" is the NTFS attribute
        // bit, not a dot-prefix. AppData/ntuser.dat/Thumbs.db are NOT dot-prefixed
        // but ARE hidden — used to slip through the filter on the NTFS fast path
        // and via watcher events. Test path_has_hidden_attribute directly because
        // a higher-level test path would be pre-filtered by DEFAULT_IGNORE_PATTERNS
        // (tempdir lives under %TEMP% = AppData on Windows).
        let dir = tempfile::Builder::new()
            .prefix("kosmos-test-")
            .tempdir()
            .unwrap();
        let visible = dir.path().join("visible.txt");
        std::fs::write(&visible, b"v").unwrap();
        let hidden = dir.path().join("ntuser-like.dat");
        std::fs::write(&hidden, b"v").unwrap();
        let status = std::process::Command::new("attrib")
            .args(["+H", hidden.to_str().unwrap()])
            .status()
            .expect("attrib must run on Windows");
        assert!(status.success(), "attrib +H failed");

        assert!(!path_has_hidden_attribute(&visible));
        assert!(path_has_hidden_attribute(&hidden));
    }
}
