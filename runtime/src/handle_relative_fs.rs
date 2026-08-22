//! Handle-relative filesystem access for package snapshots and grants.
use std::{io, path::Path};

#[derive(Clone, Debug)]
pub struct Limits {
    pub max_bytes_per_file: usize,
    pub max_total_bytes: usize,
    pub max_files: usize,
    pub max_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeFile {
    pub components: Vec<String>,
    pub bytes: Vec<u8>,
}

#[cfg_attr(
    feature = "handle-relative-fs-serde",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RootIdentity {
    pub primary: u64,
    pub secondary: u64,
}

#[derive(Debug)]
pub struct RootHandle {
    #[cfg(unix)]
    file: std::fs::File,
    #[cfg(windows)]
    handle: windows::OwnedHandle,
    #[cfg(windows)]
    identity: windows::Identity,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    BeforeChildOpen,
    AfterChildOpen,
    BeforeRead,
}
#[cfg(test)]
type FaultHook<'a> = Option<&'a dyn Fn(FaultPoint, &[String]) -> io::Result<()>>;

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
        windows::open_root(path)
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
fn walk_files_with_hook(
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
        windows::walk_files(root, &limits)
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
        windows::read_relative(root, components, max_bytes)
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
        });
    }
    #[cfg(windows)]
    {
        windows::file_identity(root, component)
    }
}

fn validate_components(components: &[&str]) -> io::Result<()> {
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty relative path",
        ));
    }
    for component in components {
        if component.is_empty()
            || *component == "."
            || *component == ".."
            || component
                .bytes()
                .any(|b| b == b'/' || b == b'\\' || b < 0x20 || b == 0x7f)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid relative component",
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn read_file(file: &std::fs::File, max: usize) -> io::Result<Vec<u8>> {
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
fn openat(parent: libc::c_int, name: &str, directory: bool) -> io::Result<libc::c_int> {
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
fn walk_dir_unix(
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
    let scan_fd = unsafe { libc::dup(directory.file.as_raw_fd()) };
    if scan_fd < 0 {
        return Err(io::Error::last_os_error());
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedDirectoryRecord {
    name: String,
    directory: bool,
    reparse_tag: u32,
    file_id: [u8; 16],
}

fn parse_directory_record(
    buffer: &[u8],
    used: usize,
    offset: usize,
) -> io::Result<(ParsedDirectoryRecord, Option<usize>)> {
    const HEADER: usize = 88;
    if offset > used || used - offset < HEADER {
        return Err(io::Error::other("short directory record"));
    }
    let p = &buffer[offset..used];
    let next = u32::from_ne_bytes(
        p[0..4]
            .try_into()
            .map_err(|_| io::Error::other("invalid directory offset header"))?,
    ) as usize;
    if next != 0 && (next < HEADER || next % 8 != 0 || next > p.len()) {
        return Err(io::Error::other("invalid directory offset"));
    }
    let attrs = u32::from_ne_bytes(
        p[56..60]
            .try_into()
            .map_err(|_| io::Error::other("invalid directory attributes"))?,
    );
    let name_len = u32::from_ne_bytes(
        p[60..64]
            .try_into()
            .map_err(|_| io::Error::other("invalid directory name length"))?,
    ) as usize;
    let reparse_tag = u32::from_ne_bytes(
        p[68..72]
            .try_into()
            .map_err(|_| io::Error::other("invalid directory reparse tag"))?,
    );
    let name_end = HEADER
        .checked_add(name_len)
        .ok_or_else(|| io::Error::other("filename overflow"))?;
    if !name_len.is_multiple_of(2) || name_end > p.len() {
        return Err(io::Error::other("invalid directory filename"));
    }
    let name_bytes = &p[HEADER..name_end];
    let name = String::from_utf16(
        name_bytes
            .chunks_exact(2)
            .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>()
            .as_slice(),
    )
    .map_err(|_| io::Error::other("invalid filename"))?;
    let file_id: [u8; 16] = p[72..88]
        .try_into()
        .map_err(|_| io::Error::other("invalid directory file id"))?;
    let next_offset = (next != 0).then_some(next);
    Ok((
        ParsedDirectoryRecord {
            name,
            directory: attrs & 0x10 != 0,
            reparse_tag,
            file_id,
        },
        next_offset,
    ))
}

#[cfg(test)]
mod directory_record_tests {
    use super::*;

    fn record(next: u32, attrs: u32, tag: u32, id: [u8; 16], name: &str) -> Vec<u8> {
        let words: Vec<u16> = name.encode_utf16().collect();
        let mut out = vec![0u8; 88 + words.len() * 2];
        out[0..4].copy_from_slice(&next.to_ne_bytes());
        out[56..60].copy_from_slice(&attrs.to_ne_bytes());
        out[60..64].copy_from_slice(&(words.len() as u32 * 2).to_ne_bytes());
        out[68..72].copy_from_slice(&tag.to_ne_bytes());
        out[72..88].copy_from_slice(&id);
        for (i, word) in words.iter().enumerate() {
            out[88 + i * 2..90 + i * 2].copy_from_slice(&word.to_ne_bytes());
        }
        out
    }

    #[test]
    fn parses_extended_record_and_128_bit_id() {
        let id = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let bytes = record(0, 0x10, 0, id, "файл");
        let (parsed, next) = parse_directory_record(&bytes, bytes.len(), 0).unwrap();
        assert_eq!(parsed.name, "файл");
        assert!(parsed.directory);
        assert_eq!(parsed.file_id, id);
        assert_eq!(next, None);
    }

    #[test]
    fn rejects_malformed_offsets_and_name_lengths() {
        let id = [7u8; 16];
        let mut bytes = record(0, 0, 0, id, "x");
        bytes[0..4].copy_from_slice(&87u32.to_ne_bytes());
        assert!(parse_directory_record(&bytes, bytes.len(), 0).is_err());
        let mut bytes = record(0, 0, 0, id, "x");
        bytes[60..64].copy_from_slice(&3u32.to_ne_bytes());
        assert!(parse_directory_record(&bytes, bytes.len(), 0).is_err());
        let mut bytes = record(0, 0, 0, id, "x");
        bytes[60..64].copy_from_slice(&400u32.to_ne_bytes());
        assert!(parse_directory_record(&bytes, bytes.len(), 0).is_err());
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::{c_void, OsStr},
        os::windows::ffi::OsStrExt,
    };
    pub type Handle = *mut c_void;
    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
    const FILE_LIST_DIRECTORY: u32 = 0x0001;
    const FILE_READ_DATA: u32 = 0x0001;
    const FILE_SHARE_ALL: u32 = 7;
    const FILE_OPEN: u32 = 1;
    const OPEN_EXISTING: u32 = 3;
    const FILE_DIRECTORY_FILE: u32 = 0x00000001;
    const FILE_NON_DIRECTORY_FILE: u32 = 0x00000040;
    const FILE_SYNCHRONOUS_IO_NONALERT: u32 = 0x20;
    const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const OBJ_CASE_INSENSITIVE: u32 = 0x40;
    const FILE_READ_ATTRIBUTES: u32 = 0x0000_0080;
    const SYNCHRONIZE: u32 = 0x0010_0000;
    const FILE_ID_EXTD_DIR_INFORMATION_CLASS: u32 = 60;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    const FILE_ATTRIBUTE_TAG_INFO_CLASS: u32 = 9;
    const FILE_ID_INFO_CLASS: u32 = 18;
    const FILE_STANDARD_INFO_CLASS: u32 = 1;
    const STATUS_NO_MORE_FILES: i32 = 0x80000006u32 as i32;
    const STATUS_BUFFER_OVERFLOW: i32 = 0x80000005u32 as i32;
    const STATUS_BUFFER_TOO_SMALL: i32 = 0xC0000023u32 as i32;

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }
    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: Handle,
        object_name: *mut UnicodeString,
        attributes: u32,
        security_descriptor: *mut c_void,
        security_qos: *mut c_void,
    }
    #[repr(C)]
    struct IoStatusBlock {
        status: i32,
        information: usize,
    }
    #[repr(C)]
    struct FileAttributeTagInfo {
        file_attributes: u32,
        reparse_tag: u32,
    }
    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct FileId128 {
        bytes: [u8; 16],
    }
    #[repr(C)]
    struct FileIdInfo {
        volume_serial: u64,
        file_id: FileId128,
    }
    #[repr(C)]
    struct FileStandardInfo {
        allocation_size: i64,
        end_of_file: i64,
        number_of_links: u32,
        delete_pending: u8,
        directory: u8,
    }
    #[repr(C)]
    struct Entry {
        name: String,
        directory: bool,
        reparse: bool,
        id: FileId128,
    }
    #[repr(C)]
    struct FileIdExtdDirHeader {
        next_entry_offset: u32,
        file_index: u32,
        creation_time: i64,
        last_access_time: i64,
        last_write_time: i64,
        change_time: i64,
        end_of_file: i64,
        allocation_size: i64,
        file_attributes: u32,
        file_name_length: u32,
        ea_size: u32,
        reparse_point_tag: u32,
        file_id: FileId128,
    }
    const _: () = {
        assert!(std::mem::size_of::<FileIdExtdDirHeader>() == 88);
        assert!(std::mem::offset_of!(FileIdExtdDirHeader, file_attributes) == 56);
        assert!(std::mem::offset_of!(FileIdExtdDirHeader, file_name_length) == 60);
        assert!(std::mem::offset_of!(FileIdExtdDirHeader, reparse_point_tag) == 68);
        assert!(std::mem::offset_of!(FileIdExtdDirHeader, file_id) == 72);
    };
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: Handle,
        ) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn DuplicateHandle(
            process: Handle,
            source: Handle,
            target_process: Handle,
            target: *mut Handle,
            access: u32,
            inherit: i32,
            options: u32,
        ) -> i32;
        fn ReadFile(
            handle: Handle,
            buffer: *mut u8,
            length: u32,
            read: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn GetFileInformationByHandleEx(
            handle: Handle,
            class: u32,
            info: *mut c_void,
            size: u32,
        ) -> i32;
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtCreateFile(
            file: *mut Handle,
            access: u32,
            attrs: *mut ObjectAttributes,
            iosb: *mut IoStatusBlock,
            allocation: *mut i64,
            attributes: u32,
            share: u32,
            disposition: u32,
            options: u32,
            ea: *mut c_void,
            ea_len: u32,
        ) -> i32;
        fn NtQueryDirectoryFile(
            file: Handle,
            event: Handle,
            apc: *mut c_void,
            context: *mut c_void,
            iosb: *mut IoStatusBlock,
            buffer: *mut c_void,
            length: u32,
            class: u32,
            single: u8,
            name: *mut UnicodeString,
            restart: u8,
        ) -> i32;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Identity {
        volume_serial: u64,
        file_id: FileId128,
    }
    #[derive(Debug)]
    pub struct OwnedHandle(Handle);
    // Windows HANDLE ownership can be transferred between Tokio tasks; the
    // handle itself is an OS kernel object and is not tied to a thread.
    unsafe impl Send for OwnedHandle {}
    unsafe impl Sync for OwnedHandle {}
    impl OwnedHandle {
        fn raw(&self) -> Handle {
            self.0
        }
    }
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                unsafe {
                    CloseHandle(self.0);
                }
                self.0 = std::ptr::null_mut();
            }
        }
    }
    impl RootHandle {
        pub(super) fn windows_handle(&self) -> Handle {
            self.handle.raw()
        }
    }

    impl Identity {
        pub(super) fn volume_serial(&self) -> u64 {
            self.volume_serial
        }

        pub(super) fn file_id_bytes(&self) -> &[u8; 16] {
            &self.file_id.bytes
        }
    }

    fn err(status: i32) -> io::Error {
        io::Error::other(format!("NTSTATUS 0x{status:08x}"))
    }
    fn nt(status: i32) -> io::Result<()> {
        if status < 0 {
            Err(err(status))
        } else {
            Ok(())
        }
    }
    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }
    fn query<T>(handle: Handle, class: u32) -> io::Result<T> {
        let mut value = unsafe { std::mem::zeroed::<T>() };
        let ok = unsafe {
            GetFileInformationByHandleEx(
                handle,
                class,
                (&mut value as *mut T).cast(),
                std::mem::size_of::<T>() as u32,
            )
        };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(value)
        }
    }
    fn identity(handle: Handle) -> io::Result<Identity> {
        let v: FileIdInfo = query(handle, FILE_ID_INFO_CLASS)?;
        Ok(Identity {
            volume_serial: v.volume_serial,
            file_id: v.file_id,
        })
    }
    fn validate_opened(
        handle: Handle,
        expect: Option<&Identity>,
        directory: bool,
    ) -> io::Result<Identity> {
        let tags: FileAttributeTagInfo = query(handle, FILE_ATTRIBUTE_TAG_INFO_CLASS)?;
        if tags.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 || tags.reparse_tag != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "reparse point",
            ));
        }
        let id = identity(handle)?;
        if let Some(expected) = expect {
            if expected.volume_serial != id.volume_serial
                || expected.file_id.bytes != id.file_id.bytes
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "file identity changed",
                ));
            }
        }
        if directory && tags.file_attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a directory",
            ));
        }
        if !directory && tags.file_attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
        }
        Ok(id)
    }
    pub fn open_root(path: &Path) -> io::Result<RootHandle> {
        let name = wide(path);
        let h = unsafe {
            CreateFileW(
                name.as_ptr(),
                FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
                FILE_SHARE_ALL,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let owned = OwnedHandle(h);
        let tags: FileAttributeTagInfo = query(owned.raw(), FILE_ATTRIBUTE_TAG_INFO_CLASS)?;
        if tags.file_attributes & FILE_ATTRIBUTE_DIRECTORY == 0
            || tags.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || tags.reparse_tag != 0
        {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid root"));
        }
        let id = identity(owned.raw())?;
        Ok(RootHandle {
            handle: owned,
            identity: id,
        })
    }
    fn relative(parent: Handle, name: &str, directory: bool) -> io::Result<OwnedHandle> {
        let mut w: Vec<u16> = OsStr::new(name).encode_wide().collect();
        let bytes = u16::try_from(
            w.len()
                .checked_mul(2)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "name too long"))?,
        )
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "name too long"))?;
        let mut us = UnicodeString {
            length: bytes,
            maximum_length: bytes,
            buffer: w.as_mut_ptr(),
        };
        let mut oa = ObjectAttributes {
            length: std::mem::size_of::<ObjectAttributes>() as u32,
            root_directory: parent,
            object_name: &mut us,
            attributes: OBJ_CASE_INSENSITIVE,
            security_descriptor: std::ptr::null_mut(),
            security_qos: std::ptr::null_mut(),
        };
        let mut iosb = IoStatusBlock {
            status: 0,
            information: 0,
        };
        let mut h = std::ptr::null_mut();
        let status = unsafe {
            NtCreateFile(
                &mut h,
                if directory {
                    FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE
                } else {
                    FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE
                },
                &mut oa,
                &mut iosb,
                std::ptr::null_mut(),
                0,
                FILE_SHARE_ALL,
                FILE_OPEN,
                FILE_OPEN_REPARSE_POINT
                    | FILE_SYNCHRONOUS_IO_NONALERT
                    | if directory {
                        FILE_DIRECTORY_FILE
                    } else {
                        FILE_NON_DIRECTORY_FILE
                    },
                std::ptr::null_mut(),
                0,
            )
        };
        let owned = OwnedHandle(h);
        nt(status)?;
        if owned.raw().is_null() {
            return Err(io::Error::other("null handle"));
        }
        Ok(owned)
    }
    fn enumerate(dir: Handle, max_entries: usize) -> io::Result<Vec<Entry>> {
        let mut buffer = vec![0u8; 64 * 1024];
        let mut out = Vec::new();
        let mut restart = 1u8;
        loop {
            let mut iosb = IoStatusBlock {
                status: 0,
                information: 0,
            };
            let status = unsafe {
                NtQueryDirectoryFile(
                    dir,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut iosb,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    FILE_ID_EXTD_DIR_INFORMATION_CLASS,
                    0,
                    std::ptr::null_mut(),
                    restart,
                )
            };
            restart = 0;
            if status == STATUS_NO_MORE_FILES {
                break;
            }
            if status == STATUS_BUFFER_OVERFLOW || status == STATUS_BUFFER_TOO_SMALL {
                return Err(err(status));
            }
            nt(status)?;
            let used = iosb.information.min(buffer.len());
            let mut offset = 0usize;
            while offset < used {
                let (parsed, next) = parse_directory_record(&buffer, used, offset)?;
                if parsed.reparse_tag != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "reparse point",
                    ));
                }
                if parsed.name != "." && parsed.name != ".." {
                    if out.len() >= max_entries {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "directory entry limit exceeded",
                        ));
                    }
                    validate_components(&[&parsed.name])?;
                    out.push(Entry {
                        name: parsed.name,
                        directory: parsed.directory,
                        reparse: false,
                        id: FileId128 {
                            bytes: parsed.file_id,
                        },
                    });
                }
                match next {
                    Some(step) => offset += step,
                    None => break,
                }
            }
        }
        Ok(out)
    }
    fn read_handle(handle: Handle, max: usize) -> io::Result<Vec<u8>> {
        let tags: FileAttributeTagInfo = query(handle, FILE_ATTRIBUTE_TAG_INFO_CLASS)?;
        if tags.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || tags.reparse_tag != 0
            || tags.file_attributes & FILE_ATTRIBUTE_DIRECTORY != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid readable handle",
            ));
        }
        let standard: FileStandardInfo = query(handle, FILE_STANDARD_INFO_CLASS)?;
        if standard.end_of_file < 0 || standard.end_of_file as u64 > max as u64 {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "file exceeds limit",
            ));
        }
        let mut out = Vec::with_capacity(
            usize::try_from(standard.end_of_file)
                .unwrap_or(max)
                .min(max),
        );
        let mut buf = [0u8; 64 * 1024];
        loop {
            let ask = buf
                .len()
                .min(max.saturating_sub(out.len()).saturating_add(1));
            if ask == 0 {
                break;
            }
            let mut got = 0u32;
            let ok = unsafe {
                ReadFile(
                    handle,
                    buf.as_mut_ptr(),
                    ask as u32,
                    &mut got,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            if got == 0 {
                break;
            }
            out.extend_from_slice(&buf[..got as usize]);
            if out.len() > max {
                return Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    "file exceeds limit",
                ));
            }
        }
        Ok(out)
    }
    pub fn walk_files(root: &RootHandle, limits: &Limits) -> io::Result<Vec<RelativeFile>> {
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
    pub fn read_relative(
        root: &RootHandle,
        components: &[&str],
        max: usize,
    ) -> io::Result<Vec<u8>> {
        let mut parent = relative_duplicate(root.handle.raw())?;
        for component in &components[..components.len() - 1] {
            let child = relative(parent.raw(), component, true)?;
            validate_opened(child.raw(), None, true)?;
            parent = child;
        }
        let file = relative(parent.raw(), components[components.len() - 1], false)?;
        validate_opened(file.raw(), None, false)?;
        read_handle(file.raw(), max)
    }
    pub fn file_identity(root: &RootHandle, component: &str) -> io::Result<RootIdentity> {
        let file = relative(root.handle.raw(), component, false)?;
        let identity = validate_opened(file.raw(), None, false)?;
        let mut secondary = [0u8; 8];
        secondary.copy_from_slice(&identity.file_id_bytes()[..8]);
        Ok(RootIdentity {
            primary: identity.volume_serial(),
            secondary: u64::from_le_bytes(secondary),
        })
    }
    fn relative_duplicate(handle: Handle) -> io::Result<OwnedHandle> {
        let mut duplicate = std::ptr::null_mut();
        let ok = unsafe {
            DuplicateHandle(
                INVALID_HANDLE_VALUE,
                handle,
                INVALID_HANDLE_VALUE,
                &mut duplicate,
                0,
                0,
                2,
            )
        };
        if ok == 0 || duplicate.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(OwnedHandle(duplicate))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;
    fn limits() -> Limits {
        Limits {
            max_bytes_per_file: 1024,
            max_total_bytes: 4096,
            max_files: 16,
            max_depth: 8,
        }
    }
    fn fixture() -> (TempDir, RootHandle) {
        let td = tempfile::tempdir().unwrap();
        fs::create_dir_all(td.path().join("nested/deep")).unwrap();
        fs::write(td.path().join("root.txt"), b"root").unwrap();
        fs::write(td.path().join("nested/deep/file.txt"), b"nested").unwrap();
        let root = open_root(td.path()).unwrap();
        (td, root)
    }
    #[test]
    fn walks_nested_files_without_exposing_paths() {
        let (_td, root) = fixture();
        let mut files = walk_files(&root, limits()).unwrap();
        files.sort_by(|a, b| a.components.cmp(&b.components));
        assert_eq!(files[0].components, vec!["nested", "deep", "file.txt"]);
        assert_eq!(files[1].components, vec!["root.txt"]);
        assert_eq!(files[0].bytes, b"nested");
    }
    #[test]
    fn read_relative_rejects_traversal_and_reads_by_handle() {
        let (_td, root) = fixture();
        assert_eq!(
            read_relative(&root, &["nested", "deep", "file.txt"], 100).unwrap(),
            b"nested"
        );
        for bad in [vec![".."], vec!["a/b"], vec!["a\\b"], vec!["a\0b"]] {
            assert!(read_relative(&root, &bad, 100).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn denies_symlinks_and_unsupported_entries() {
        let td = tempfile::tempdir().unwrap();
        fs::write(td.path().join("good"), b"ok").unwrap();
        symlink(td.path().join("good"), td.path().join("link-file")).unwrap();
        symlink(td.path(), td.path().join("link-dir")).unwrap();
        std::os::unix::net::UnixListener::bind(td.path().join("socket")).unwrap();
        let root = open_root(td.path()).unwrap();
        assert!(walk_files(&root, limits()).is_err());
    }
    #[test]
    fn enforces_size_count_and_depth_caps() {
        let td = tempfile::tempdir().unwrap();
        fs::create_dir_all(td.path().join("a/b/c")).unwrap();
        fs::write(td.path().join("a/b/c/file"), vec![0u8; 20]).unwrap();
        let root = open_root(td.path()).unwrap();
        let mut capped = limits();
        capped.max_bytes_per_file = 10;
        assert!(walk_files(&root, capped).is_err(), "size");
        let mut capped = limits();
        capped.max_depth = 0;
        assert!(walk_files(&root, capped).is_err(), "depth");
        fs::write(td.path().join("one"), b"1").unwrap();
        let mut capped = limits();
        capped.max_files = 0;
        assert!(walk_files(&root, capped).is_err(), "count");
    }
    #[test]
    fn root_replacement_stays_on_open_handle() {
        let td = tempfile::tempdir().unwrap();
        fs::create_dir_all(td.path().join("nested")).unwrap();
        fs::write(td.path().join("nested/file"), b"old").unwrap();
        let root = open_root(td.path()).unwrap();
        fs::rename(td.path(), td.path().with_extension("old")).unwrap();
        fs::create_dir_all(td.path().join("nested")).unwrap();
        fs::write(td.path().join("nested/file"), b"new").unwrap();
        assert_eq!(
            read_relative(&root, &["nested", "file"], 100).unwrap(),
            b"old"
        );
    }
    #[cfg(unix)]
    #[test]
    fn fault_hook_and_read_failure_are_observed() {
        let (_td, root) = fixture();
        let hook = |point: FaultPoint, _path: &[String]| {
            if point == FaultPoint::BeforeRead {
                Err(io::Error::other("injected"))
            } else {
                Ok(())
            }
        };
        assert!(walk_files_with_hook(&root, limits(), Some(&hook)).is_err());
        let mut capped = limits();
        capped.max_total_bytes = 0;
        assert!(walk_files_with_hook(&root, capped, Some(&|_, _| Ok(()))).is_err());
    }
    #[test]
    fn child_swap_hook_never_returns_attacker_bytes() {
        use std::cell::Cell;
        let td = tempfile::tempdir().unwrap();
        fs::write(td.path().join("victim"), b"original").unwrap();
        let root = open_root(td.path()).unwrap();
        let moved = td.path().join("victim.held");
        let attacker = td.path().join("victim");
        let swapped = Cell::new(false);
        let hook = |point: FaultPoint, path: &[String]| {
            if path == ["victim"] && point == FaultPoint::AfterChildOpen && !swapped.get() {
                fs::rename(&attacker, &moved)?;
                fs::write(&attacker, b"attacker")?;
                swapped.set(true);
            } else if path == ["victim"] && point == FaultPoint::BeforeRead && swapped.get() {
                fs::remove_file(&attacker)?;
                fs::rename(&moved, &attacker)?;
            }
            Ok(())
        };
        let files = walk_files_with_hook(&root, limits(), Some(&hook)).unwrap();
        assert_eq!(
            files
                .iter()
                .find(|f| f.components == ["victim"])
                .unwrap()
                .bytes,
            b"original"
        );
    }

    #[cfg(unix)]
    #[test]
    fn repeated_malformed_walks_do_not_leak_fds() {
        let (_td, root) = fixture();
        let before = fs::read_dir("/proc/self/fd").unwrap().count();
        for _ in 0..100 {
            let mut capped = limits();
            capped.max_files = 0;
            assert!(walk_files(&root, capped).is_err());
        }
        let after = fs::read_dir("/proc/self/fd").unwrap().count();
        assert!(
            after <= before + 2,
            "fd leak: before={before} after={after}"
        );
    }
}
