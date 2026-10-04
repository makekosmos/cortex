use super::*;
use std::sync::atomic::Ordering;

#[cfg(unix)]
pub(super) fn read_file(file: &std::fs::File, max: usize) -> io::Result<Vec<u8>> {
    use std::io::Read;
    if file.metadata()?.len() > max as u64 {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "file exceeds limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "file exceeds limit",
        ));
    }
    Ok(bytes)
}

#[cfg(unix)]
pub(super) fn openat(parent: libc::c_int, name: &str, directory: bool) -> io::Result<libc::c_int> {
    use std::ffi::CString;
    let name = CString::new(name)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nul component"))?;
    let mut flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW;
    if directory {
        flags |= libc::O_DIRECTORY;
    }
    let fd = unsafe { libc::openat(parent, name.as_ptr(), flags) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

#[cfg(unix)]
fn openat_create(parent: libc::c_int, name: &str) -> io::Result<libc::c_int> {
    use std::ffi::CString;
    let name = CString::new(name)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nul component"))?;
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

#[cfg(unix)]
pub(super) fn open_directory_relative(
    root: &RootHandle,
    components: &[&str],
) -> io::Result<std::fs::File> {
    use std::os::unix::io::{AsRawFd, FromRawFd};
    let mut fd = unsafe { libc::dup(root.file.as_raw_fd()) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    for component in components {
        let child = match openat(fd, component, true) {
            Ok(child) => child,
            Err(error) => {
                unsafe { libc::close(fd) };
                return Err(error);
            }
        };
        unsafe { libc::close(fd) };
        fd = child;
    }
    Ok(unsafe { std::fs::File::from_raw_fd(fd) })
}

#[cfg(unix)]
fn unix_file_identity(file: &std::fs::File) -> io::Result<RootIdentity> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    Ok(RootIdentity {
        primary: metadata.dev(),
        secondary: metadata.ino(),
    })
}

#[cfg(unix)]
fn existing_file_identity(parent: libc::c_int, name: &str) -> io::Result<Option<RootIdentity>> {
    use std::os::unix::io::FromRawFd;
    match openat(parent, name, false) {
        Ok(fd) => {
            let file = unsafe { std::fs::File::from_raw_fd(fd) };
            unix_file_identity(&file).map(Some)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn unlinkat(parent: libc::c_int, name: &str) -> io::Result<()> {
    use std::ffi::CString;
    let name = CString::new(name)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nul component"))?;
    let result = unsafe { libc::unlinkat(parent, name.as_ptr(), 0) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
pub(super) fn list_relative_unix(
    root: &RootHandle,
    components: &[&str],
    max_entries: usize,
) -> io::Result<Vec<RelativeEntry>> {
    use std::{
        ffi::CStr,
        os::unix::io::{AsRawFd, FromRawFd},
    };
    if !components.is_empty() {
        validate_components(components)?;
    }
    let directory = open_directory_relative(root, components)?;
    // `dup` shares the open file description's directory offset, so scanning
    // a duplicate would consume the persistent root's stream position and a
    // repeated listing would start at EOF. Opening "." yields a fresh fd and
    // stream position instead.
    let scan_fd = openat(directory.as_raw_fd(), ".", true)?;
    let dir = unsafe { libc::fdopendir(scan_fd) };
    if dir.is_null() {
        unsafe { libc::close(scan_fd) };
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        let mut entries = Vec::new();
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
            if entries.len() >= max_entries {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "directory entry limit exceeded",
                ));
            }
            let fd = openat(directory.as_raw_fd(), name, false)?;
            let child = unsafe { std::fs::File::from_raw_fd(fd) };
            let metadata = child.metadata()?;
            if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "unsupported or symlink entry",
                ));
            }
            entries.push(RelativeEntry {
                name: name.to_owned(),
                directory: metadata.is_dir(),
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    })();
    unsafe { libc::closedir(dir) };
    result
}

#[cfg(unix)]
pub(super) fn write_relative_unix(
    root: &RootHandle,
    components: &[&str],
    bytes: &[u8],
) -> io::Result<()> {
    use std::{
        io::Write,
        os::unix::io::{AsRawFd, FromRawFd},
    };
    let parent = open_directory_relative(root, &components[..components.len() - 1])?;
    let parent_fd = parent.as_raw_fd();
    let target = components[components.len() - 1];
    let before = existing_file_identity(parent_fd, target)?;
    let mut temp_name = None;
    let mut temp = None;
    for _ in 0..128 {
        let candidate = format!(
            ".mundus-grant-{}-{}.tmp",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        match openat_create(parent_fd, &candidate) {
            Ok(fd) => {
                temp_name = Some(candidate);
                temp = Some(unsafe { std::fs::File::from_raw_fd(fd) });
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    let temp_name = temp_name
        .ok_or_else(|| io::Error::new(io::ErrorKind::AlreadyExists, "temporary file collision"))?;
    let mut temp = temp.expect("temporary file is set with its name");
    let result = (|| {
        temp.write_all(bytes)?;
        temp.sync_all()?;
        let after = existing_file_identity(parent_fd, target)?;
        if after != before {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "target identity changed",
            ));
        }
        use std::ffi::CString;
        let source = CString::new(temp_name.as_str()).expect("generated name has no nul");
        let target = CString::new(target)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nul component"))?;
        if unsafe { libc::renameat(parent_fd, source.as_ptr(), parent_fd, target.as_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        parent.sync_all()
    })();
    drop(temp);
    if result.is_err() {
        let _ = unlinkat(parent_fd, &temp_name);
    }
    result
}

#[cfg(unix)]
pub(super) fn delete_relative_unix(root: &RootHandle, components: &[&str]) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let parent = open_directory_relative(root, &components[..components.len() - 1])?;
    let target = components[components.len() - 1];
    let _ = existing_file_identity(parent.as_raw_fd(), target)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "file not found"))?;
    unlinkat(parent.as_raw_fd(), target)
}

#[cfg(unix)]
pub(super) fn mkdir_relative_unix(root: &RootHandle, components: &[&str]) -> io::Result<()> {
    use std::{
        ffi::CString,
        os::unix::io::{AsRawFd, FromRawFd},
    };
    let mut parent = open_directory_relative(root, &[])?;
    for component in components {
        let name = CString::new(*component)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nul component"))?;
        let result = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
        if result != 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
        let fd = openat(parent.as_raw_fd(), component, true)?;
        parent = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    Ok(())
}
