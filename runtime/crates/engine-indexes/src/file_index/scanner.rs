use super::{FileIndexError, IndexedFile, NtfsStatus, Result, ScanOptions};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
#[cfg(windows)]
mod ntfs;

mod filter;
mod roots;
#[cfg(windows)]
use filter::is_drive_root;
#[cfg(all(windows, test))]
use filter::path_has_hidden_attribute;
use filter::{filter_indexed_files, ignore_matcher, should_enter_dir, should_index_path_for_root};
pub use filter::{path_contains_noisy_folder, should_index_with_options, validate_ignore_pattern};
pub use roots::{default_roots, open_file};

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

#[cfg(test)]
#[path = "scanner/tests.rs"]
mod tests;
