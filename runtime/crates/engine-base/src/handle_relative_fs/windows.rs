use super::{io, RootHandle};
use std::{
    ffi::{c_void, OsStr},
    os::windows::ffi::OsStrExt,
    path::Path,
};

mod enumerate;
pub(super) mod open;
pub(super) mod ops;
pub(super) mod walk;
pub(super) mod write;

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
const DELETE: u32 = 0x0001_0000;
const FILE_ADD_SUBDIRECTORY: u32 = 0x0004;
const FILE_CREATE: u32 = 2;
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
