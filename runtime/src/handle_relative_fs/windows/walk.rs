use super::enumerate::{enumerate, enumerate_impl};
use super::open::{read_handle, relative, validate_opened};
use super::{query, FileStandardInfo, Handle, Identity, FILE_STANDARD_INFO_CLASS};
use crate::handle_relative_fs::{Limits, RelativeFile, RootHandle, TreeEntry};
use std::io;

pub(in crate::handle_relative_fs) fn walk_files(
    root: &RootHandle,
    limits: &Limits,
) -> io::Result<Vec<RelativeFile>> {
    fn walk(
        dir: Handle,
        prefix: &mut Vec<String>,
        depth: usize,
        limits: &Limits,
        total: &mut usize,
        out: &mut Vec<RelativeFile>,
        root_identity: &Identity,
    ) -> io::Result<()> {
        if depth > limits.max_depth {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "directory depth exceeded",
            ));
        }
        for entry in enumerate(dir, limits.max_files)? {
            prefix.push(entry.name.clone());
            let result = (|| {
                if entry.reparse {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "reparse point",
                    ));
                }
                if entry.directory {
                    let child = relative(dir, &entry.name, true)?;
                    let id = validate_opened(
                        child.raw(),
                        Some(&Identity {
                            volume_serial: root_identity.volume_serial,
                            file_id: entry.id,
                        }),
                        true,
                    )?;
                    let _ = id;
                    walk(
                        child.raw(),
                        prefix,
                        depth + 1,
                        limits,
                        total,
                        out,
                        root_identity,
                    )
                } else {
                    if out.len() >= limits.max_files {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "file count exceeded",
                        ));
                    }
                    let child = relative(dir, &entry.name, false)?;
                    let _ = validate_opened(
                        child.raw(),
                        Some(&Identity {
                            volume_serial: root_identity.volume_serial,
                            file_id: entry.id,
                        }),
                        false,
                    )?;
                    let bytes = read_handle(child.raw(), limits.max_bytes_per_file)?;
                    *total = total.checked_add(bytes.len()).ok_or_else(|| {
                        io::Error::new(io::ErrorKind::FileTooLarge, "total size overflow")
                    })?;
                    if *total > limits.max_total_bytes {
                        return Err(io::Error::new(
                            io::ErrorKind::FileTooLarge,
                            "total size exceeded",
                        ));
                    }
                    out.push(RelativeFile {
                        components: prefix.clone(),
                        bytes,
                    });
                    Ok(())
                }
            })();
            prefix.pop();
            result?;
        }
        Ok(())
    }
    let mut out = Vec::new();
    let mut total = 0usize;
    walk(
        root.handle.raw(),
        &mut Vec::new(),
        0,
        limits,
        &mut total,
        &mut out,
        &root.identity,
    )?;
    out.sort_by(|a, b| a.components.cmp(&b.components));
    Ok(out)
}
pub(in crate::handle_relative_fs) fn walk_entries(
    root: &RootHandle,
    max_depth: usize,
    max_entries: usize,
    skip_dir: &dyn Fn(&str) -> bool,
) -> io::Result<Vec<TreeEntry>> {
    fn walk(
        dir: Handle,
        prefix: &mut Vec<String>,
        depth: usize,
        max_depth: usize,
        max_entries: usize,
        skip_dir: &dyn Fn(&str) -> bool,
        out: &mut Vec<TreeEntry>,
        root_identity: &Identity,
    ) -> io::Result<()> {
        if depth > max_depth {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "directory depth exceeded",
            ));
        }
        for entry in enumerate_impl(dir, max_entries, true)? {
            // Dirent.isFile()/isDirectory() parity: reparse points
            // (symlinks/junctions) are skipped instead of failing the scan.
            if entry.reparse {
                continue;
            }
            prefix.push(entry.name.clone());
            let result = (|| {
                if entry.directory {
                    if skip_dir(&entry.name) {
                        return Ok(());
                    }
                    let child = relative(dir, &entry.name, true)?;
                    let _ = validate_opened(
                        child.raw(),
                        Some(&Identity {
                            volume_serial: root_identity.volume_serial,
                            file_id: entry.id,
                        }),
                        true,
                    )?;
                    walk(
                        child.raw(),
                        prefix,
                        depth + 1,
                        max_depth,
                        max_entries,
                        skip_dir,
                        out,
                        root_identity,
                    )
                } else {
                    if out.len() >= max_entries {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "file count exceeded",
                        ));
                    }
                    let child = relative(dir, &entry.name, false)?;
                    let _ = validate_opened(
                        child.raw(),
                        Some(&Identity {
                            volume_serial: root_identity.volume_serial,
                            file_id: entry.id,
                        }),
                        false,
                    )?;
                    let standard: FileStandardInfo = query(child.raw(), FILE_STANDARD_INFO_CLASS)?;
                    if standard.end_of_file < 0 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "invalid file size",
                        ));
                    }
                    out.push(TreeEntry {
                        components: prefix.clone(),
                        size: standard.end_of_file as u64,
                    });
                    Ok(())
                }
            })();
            prefix.pop();
            result?;
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(
        root.handle.raw(),
        &mut Vec::new(),
        0,
        max_depth,
        max_entries,
        skip_dir,
        &mut out,
        &root.identity,
    )?;
    out.sort_by(|a, b| a.components.cmp(&b.components));
    Ok(out)
}
