#[cfg(test)]
use super::FaultHook;
use super::{Limits, RelativeFile, RootHandle, TreeEntry};
use std::io;

#[cfg(unix)]
use super::unix_walk::{walk_dir_unix, walk_entries_unix};

pub fn walk_entries(
    root: &RootHandle,
    max_depth: usize,
    max_entries: usize,
    skip_dir: &dyn Fn(&str) -> bool,
) -> io::Result<Vec<TreeEntry>> {
    #[cfg(unix)]
    {
        let mut out = Vec::new();
        walk_entries_unix(
            root,
            &mut Vec::new(),
            0,
            max_depth,
            max_entries,
            skip_dir,
            &mut out,
        )?;
        out.sort_by(|a, b| a.components.cmp(&b.components));
        Ok(out)
    }
    #[cfg(windows)]
    {
        super::windows::walk::walk_entries(root, max_depth, max_entries, skip_dir)
    }
}

pub fn walk_files(root: &RootHandle, limits: Limits) -> io::Result<Vec<RelativeFile>> {
    #[cfg(test)]
    {
        walk_files_inner(root, limits, None)
    }
    #[cfg(not(test))]
    {
        walk_files_inner(root, limits, ())
    }
}

#[cfg(test)]
pub(super) fn walk_files_with_hook(
    root: &RootHandle,
    limits: Limits,
    hook: FaultHook<'_>,
) -> io::Result<Vec<RelativeFile>> {
    walk_files_inner(root, limits, hook)
}

fn walk_files_inner(
    root: &RootHandle,
    limits: Limits,
    #[cfg(all(test, unix))] hook: FaultHook<'_>,
    #[cfg(all(test, windows))] _hook: FaultHook<'_>,
    #[cfg(not(test))] _hook: (),
) -> io::Result<Vec<RelativeFile>> {
    if limits.max_files == 0 || limits.max_depth == 0 || limits.max_total_bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "traversal limit exceeded",
        ));
    }
    #[cfg(unix)]
    {
        let mut files = Vec::new();
        let mut total = 0usize;
        walk_dir_unix(
            root,
            &mut Vec::new(),
            0,
            &limits,
            &mut total,
            &mut files,
            #[cfg(test)]
            hook,
        )?;
        files.sort_by(|a, b| a.components.cmp(&b.components));
        Ok(files)
    }
    #[cfg(windows)]
    {
        super::windows::walk::walk_files(root, &limits)
    }
}
