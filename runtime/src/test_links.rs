//! Shared test helpers for creating real filesystem links.
//!
//! Security tests must never silently pass because the environment could not
//! produce a link, so every helper returns `io::Result` and callers `.expect()`
//! it. On Windows a directory link is an NTFS junction written through
//! `FSCTL_SET_REPARSE_POINT`: `mklink /J` needs a cmd round-trip whose quoting
//! Rust's `Command` arguments break, and `symlink_dir` requires
//! SeCreateSymbolicLinkPrivilege or Developer Mode (error 1314). A junction
//! needs neither.
use std::io;
use std::path::Path;

/// Create a directory link at `link` pointing at `target`: a symlink on
/// unix, an NTFS junction on Windows.
pub fn link_dir(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        windows::junction(target, link)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "directory links unsupported",
        ))
    }
}

/// Create a file link at `link` pointing at `target`. unix only: on Windows
/// a file symlink always needs SeCreateSymbolicLinkPrivilege, so Windows
/// tests build the same leaf-reparse-point case from a junctioned directory
/// instead of calling this.
#[cfg(unix)]
pub fn link_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    type Handle = *mut std::ffi::c_void;
    const INVALID_HANDLE: Handle = -1isize as Handle;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const OPEN_EXISTING: u32 = 3;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00a4;
    const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xa000_0003;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut std::ffi::c_void,
            disposition: u32,
            flags: u32,
            template: Handle,
        ) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn DeviceIoControl(
            handle: Handle,
            code: u32,
            input: *const std::ffi::c_void,
            input_len: u32,
            output: *mut std::ffi::c_void,
            output_len: u32,
            returned: *mut u32,
            overlapped: *mut std::ffi::c_void,
        ) -> i32;
    }

    fn wide(text: &str) -> Vec<u16> {
        OsStr::new(text).encode_wide().chain(Some(0)).collect()
    }

    /// Write a mount-point reparse point on the empty directory `link`.
    /// `link` must not exist yet — this creates the directory and then turns
    /// it into a junction, mirroring `mklink /J` semantics.
    pub fn junction(target: &Path, link: &Path) -> io::Result<()> {
        let target = std::fs::canonicalize(target)?;
        let text = target.to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "junction target is not UTF-8")
        })?;
        let bare = text.strip_prefix(r"\\?\").unwrap_or(text);
        // SubstituteName is the NT-namespace path (`\??\X:\...`); PrintName
        // is the Win32 path shown to the caller.
        let substitute = format!(r"\??\{bare}\");
        let subst_utf16: Vec<u16> = OsStr::new(&substitute).encode_wide().collect();
        let print_utf16: Vec<u16> = OsStr::new(bare).encode_wide().collect();

        // REPARSE_DATA_BUFFER: header (tag, data length, reserved) then the
        // mount-point fields and both NUL-separated UTF-16 path strings.
        let data_len = 8 + (subst_utf16.len() + 1 + print_utf16.len() + 1) * 2;
        let mut buffer = Vec::with_capacity(8 + data_len);
        buffer.extend_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
        buffer.extend_from_slice(&(data_len as u16).to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&((subst_utf16.len() * 2) as u16).to_le_bytes());
        buffer.extend_from_slice(&(((subst_utf16.len() + 1) * 2) as u16).to_le_bytes());
        buffer.extend_from_slice(&((print_utf16.len() * 2) as u16).to_le_bytes());
        for name in [&subst_utf16, &print_utf16] {
            for unit in name {
                buffer.extend_from_slice(&unit.to_le_bytes());
            }
            buffer.extend_from_slice(&0u16.to_le_bytes());
        }

        std::fs::create_dir(link)?;
        let name = wide(&link.to_string_lossy());
        let handle = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_WRITE,
                0,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            )
        };
        if handle.is_null() || handle == INVALID_HANDLE {
            let _ = std::fs::remove_dir(link);
            return Err(io::Error::last_os_error());
        }
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_SET_REPARSE_POINT,
                buffer.as_ptr().cast(),
                buffer.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        unsafe {
            CloseHandle(handle);
        }
        if ok == 0 {
            let error = io::Error::last_os_error();
            let _ = std::fs::remove_dir(link);
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn link_dir_creates_a_real_reparse_point() {
        let base = tempfile::tempdir().unwrap();
        let target = base.path().join("target");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("inside.txt"), b"data").unwrap();
        let link = base.path().join("link");
        link_dir(&target, &link).expect("junction creation must work unprivileged");

        // A junction is a name-surrogate reparse point: it must report as a
        // link, resolve to the target, and carry the target's children.
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::canonicalize(&link).unwrap(),
            fs::canonicalize(&target).unwrap()
        );
        assert_eq!(fs::read(link.join("inside.txt")).unwrap(), b"data");

        // remove_dir deletes the reparse point itself, never the target.
        fs::remove_dir(&link).unwrap();
        assert!(target.join("inside.txt").exists());
    }
}
