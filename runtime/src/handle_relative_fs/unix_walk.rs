use super::unix::*;
use super::*;

#[cfg(unix)]
pub(super) fn walk_dir_unix(
    directory: &RootHandle,
    prefix: &mut Vec<String>,
    depth: usize,
    limits: &Limits,
    total: &mut usize,
    files: &mut Vec<RelativeFile>,
    #[cfg(test)] hook: FaultHook<'_>,
) -> io::Result<()> {
    use std::{
        ffi::CStr,
        os::unix::io::{AsRawFd, FromRawFd},
    };
    if depth > limits.max_depth {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "directory depth exceeded",
        ));
    }
    // `dup` shares the directory stream offset with the original open
    // description, so re-walking the same root twice would start at EOF.
    // Opening "." yields a fresh fd and stream position instead.
    let scan_fd = openat(directory.file.as_raw_fd(), ".", true)?;
    let dir = unsafe { libc::fdopendir(scan_fd) };
    if dir.is_null() {
        unsafe {
            libc::close(scan_fd);
        }
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        loop {
            let entry = unsafe { libc::readdir(dir) };
            if entry.is_null() {
                break;
            }
            let raw = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if raw == b"." || raw == b".." {
                continue;
            }
            let name = std::str::from_utf8(raw).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "filename is not UTF-8")
            })?;
            validate_components(&[name])?;
            prefix.push(name.to_owned());
            let item = (|| {
                if prefix.len() > limits.max_depth {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "directory depth exceeded",
                    ));
                }
                #[cfg(test)]
                if let Some(h) = hook {
                    h(FaultPoint::BeforeChildOpen, prefix)?;
                }
                let fd = openat(directory.file.as_raw_fd(), name, false)?;
                let child = unsafe { std::fs::File::from_raw_fd(fd) };
                #[cfg(test)]
                if let Some(h) = hook {
                    h(FaultPoint::AfterChildOpen, prefix)?;
                }
                let meta = child.metadata()?;
                if meta.file_type().is_symlink() || (!meta.is_dir() && !meta.is_file()) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "unsupported or symlink entry",
                    ));
                }
                if meta.is_dir() {
                    let child_root = RootHandle { file: child };
                    walk_dir_unix(
                        &child_root,
                        prefix,
                        depth + 1,
                        limits,
                        total,
                        files,
                        #[cfg(test)]
                        hook,
                    )
                } else {
                    if files.len() >= limits.max_files {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "file count exceeded",
                        ));
                    }
                    #[cfg(test)]
                    if let Some(h) = hook {
                        h(FaultPoint::BeforeRead, prefix)?;
                    }
                    let bytes = read_file(&child, limits.max_bytes_per_file)?;
                    *total = total.checked_add(bytes.len()).ok_or_else(|| {
                        io::Error::new(io::ErrorKind::FileTooLarge, "total size overflow")
                    })?;
                    if *total > limits.max_total_bytes {
                        return Err(io::Error::new(
                            io::ErrorKind::FileTooLarge,
                            "total size exceeded",
                        ));
                    }
                    files.push(RelativeFile {
                        components: prefix.clone(),
                        bytes,
                    });
                    Ok(())
                }
            })();
            prefix.pop();
            item?;
        }
        Ok(())
    })();
    unsafe {
        libc::closedir(dir);
    }
    result
}

#[cfg(unix)]
pub(super) fn walk_entries_unix(
    directory: &RootHandle,
    prefix: &mut Vec<String>,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    skip_dir: &dyn Fn(&str) -> bool,
    out: &mut Vec<TreeEntry>,
) -> io::Result<()> {
    use std::{
        ffi::CStr,
        os::unix::io::{AsRawFd, FromRawFd},
    };
    if depth > max_depth {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "directory depth exceeded",
        ));
    }
    // `dup` shares the directory stream offset with the original open
    // description, so re-walking the same root twice would start at EOF.
    // Opening "." yields a fresh fd and stream position instead.
    let scan_fd = openat(directory.file.as_raw_fd(), ".", true)?;
    let dir = unsafe { libc::fdopendir(scan_fd) };
    if dir.is_null() {
        unsafe {
            libc::close(scan_fd);
        }
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        loop {
            let entry = unsafe { libc::readdir(dir) };
            if entry.is_null() {
                break;
            }
            let raw = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if raw == b"." || raw == b".." {
                continue;
            }
            let name = std::str::from_utf8(raw).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidInput, "filename is not UTF-8")
            })?;
            validate_components(&[name])?;
            let d_type = unsafe { (*entry).d_type };
            // Dirent.isFile()/isDirectory() parity: links and exotic node
            // types never get opened, so they cannot redirect the walk.
            if d_type == libc::DT_LNK
                || !matches!(d_type, libc::DT_DIR | libc::DT_REG | libc::DT_UNKNOWN)
            {
                continue;
            }
            prefix.push(name.to_owned());
            let item = (|| -> io::Result<()> {
                let is_dir = match d_type {
                    libc::DT_DIR => true,
                    libc::DT_REG => false,
                    _ => {
                        // Filesystems without d_type reporting need one guarded
                        // open to classify; ELOOP/not-found entries are skipped.
                        let fd = openat(directory.file.as_raw_fd(), name, false);
                        match fd {
                            Ok(fd) => {
                                let file = unsafe { std::fs::File::from_raw_fd(fd) };
                                let meta = file.metadata()?;
                                if meta.file_type().is_symlink()
                                    || (!meta.is_dir() && !meta.is_file())
                                {
                                    return Ok(());
                                }
                                meta.is_dir()
                            }
                            Err(error)
                                if error.kind() == io::ErrorKind::NotFound
                                    || error.raw_os_error() == Some(libc::ELOOP) =>
                            {
                                return Ok(());
                            }
                            Err(error) => return Err(error),
                        }
                    }
                };
                if is_dir {
                    if skip_dir(name) {
                        return Ok(());
                    }
                    if prefix.len() > max_depth {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "directory depth exceeded",
                        ));
                    }
                    let fd = openat(directory.file.as_raw_fd(), name, true)?;
                    let child = RootHandle {
                        file: unsafe { std::fs::File::from_raw_fd(fd) },
                    };
                    return walk_entries_unix(
                        &child,
                        prefix,
                        depth + 1,
                        max_depth,
                        max_entries,
                        skip_dir,
                        out,
                    );
                }
                if out.len() >= max_entries {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "file count exceeded",
                    ));
                }
                let fd = openat(directory.file.as_raw_fd(), name, false)?;
                let file = unsafe { std::fs::File::from_raw_fd(fd) };
                let meta = file.metadata()?;
                if !meta.is_file() || meta.file_type().is_symlink() {
                    return Ok(());
                }
                out.push(TreeEntry {
                    components: prefix.clone(),
                    size: meta.len(),
                });
                Ok(())
            })();
            prefix.pop();
            item?;
        }
        Ok(())
    })();
    unsafe {
        libc::closedir(dir);
    }
    result
}
