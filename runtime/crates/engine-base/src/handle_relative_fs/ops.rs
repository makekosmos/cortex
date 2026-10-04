use super::{validate_components, RelativeEntry, RootHandle, RootIdentity};
use std::{io, path::Path};

#[cfg(unix)]
use super::unix::{
    delete_relative_unix, list_relative_unix, mkdir_relative_unix, open_directory_relative, openat,
    read_file, write_relative_unix,
};

pub fn open_root(path: &Path) -> io::Result<RootHandle> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "root is not a directory",
            ));
        }
        Ok(RootHandle { file })
    }
    #[cfg(windows)]
    {
        super::windows::open::open_root(path)
    }
}

pub fn root_identity(root: &RootHandle) -> io::Result<RootIdentity> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = root.file.metadata()?;
        Ok(RootIdentity {
            primary: metadata.dev(),
            secondary: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        let id = root.identity;
        let mut secondary = [0u8; 8];
        secondary.copy_from_slice(&id.file_id_bytes()[..8]);
        Ok(RootIdentity {
            primary: id.volume_serial(),
            secondary: u64::from_le_bytes(secondary),
        })
    }
}

pub fn read_relative(
    root: &RootHandle,
    components: &[&str],
    max_bytes: usize,
) -> io::Result<Vec<u8>> {
    validate_components(components)?;
    #[cfg(unix)]
    {
        use std::os::unix::io::{AsRawFd, FromRawFd};
        let mut parent = unsafe { libc::dup(root.file.as_raw_fd()) };
        if parent < 0 {
            return Err(io::Error::last_os_error());
        }
        let result = (|| {
            for component in &components[..components.len() - 1] {
                let child = openat(parent, component, true)?;
                unsafe {
                    libc::close(parent);
                }
                parent = child;
            }
            let fd = openat(parent, components[components.len() - 1], false)?;
            let file = unsafe { std::fs::File::from_raw_fd(fd) };
            let meta = file.metadata()?;
            if !meta.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "not a regular file",
                ));
            }
            read_file(&file, max_bytes)
        })();
        unsafe {
            libc::close(parent);
        }
        result
    }
    #[cfg(windows)]
    {
        super::windows::ops::read_relative(root, components, max_bytes)
    }
}

pub fn list_relative(
    root: &RootHandle,
    components: &[&str],
    max_entries: usize,
) -> io::Result<Vec<RelativeEntry>> {
    if max_entries == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "entry limit exceeded",
        ));
    }
    #[cfg(unix)]
    {
        list_relative_unix(root, components, max_entries)
    }
    #[cfg(windows)]
    {
        super::windows::ops::list_relative(root, components, max_entries)
    }
}

pub fn write_relative(
    root: &RootHandle,
    components: &[&str],
    bytes: &[u8],
    max_bytes: usize,
) -> io::Result<()> {
    validate_components(components)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "file exceeds limit",
        ));
    }
    #[cfg(unix)]
    {
        write_relative_unix(root, components, bytes)
    }
    #[cfg(windows)]
    {
        super::windows::write::write_relative(root, components, bytes, max_bytes)
    }
}

pub fn delete_relative(root: &RootHandle, components: &[&str]) -> io::Result<()> {
    validate_components(components)?;
    #[cfg(unix)]
    {
        delete_relative_unix(root, components)
    }
    #[cfg(windows)]
    {
        super::windows::ops::delete_relative(root, components)
    }
}

pub fn mkdir_relative(root: &RootHandle, components: &[&str]) -> io::Result<()> {
    validate_components(components)?;
    #[cfg(unix)]
    {
        mkdir_relative_unix(root, components)
    }
    #[cfg(windows)]
    {
        super::windows::ops::mkdir_relative(root, components)
    }
}

/// Return the byte size of a regular file beneath `root`.
///
/// Every component resolves relative to an already validated directory handle
/// and the file is measured on its open handle, so a swapped parent, reparse
/// point, or replaced file cannot redirect the stat outside `root`.
pub fn stat_relative(root: &RootHandle, components: &[&str]) -> io::Result<u64> {
    validate_components(components)?;
    #[cfg(unix)]
    {
        use std::os::unix::io::{AsRawFd, FromRawFd};
        let parent = open_directory_relative(root, &components[..components.len() - 1])?;
        let fd = openat(parent.as_raw_fd(), components[components.len() - 1], false)?;
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        Ok(metadata.len())
    }
    #[cfg(windows)]
    {
        super::windows::ops::stat_relative(root, components)
    }
}

/// Return the stable identity of a regular file directly below `root`.
///
/// The file is opened relative to the already validated root handle so a
/// replacement or reparse point cannot change what is identified between
/// validation and use.
pub fn file_identity(root: &RootHandle, component: &str) -> io::Result<RootIdentity> {
    validate_components(&[component])?;
    #[cfg(unix)]
    {
        use std::os::unix::io::{AsRawFd, FromRawFd};
        let fd = openat(root.file.as_raw_fd(), component, false)?;
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            ));
        }
        use std::os::unix::fs::MetadataExt;
        Ok(RootIdentity {
            primary: metadata.dev(),
            secondary: metadata.ino(),
        })
    }
    #[cfg(windows)]
    {
        super::windows::ops::file_identity(root, component)
    }
}
