//! Broker filesystem writes: bounded temp files and atomic replace.

use super::*;
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

pub(super) fn create_temp_file(
    parent: &Path,
    _parent_handle: Option<&File>,
) -> Result<(PathBuf, File), BrokerError> {
    for _ in 0..MAX_TEMPFILE_ATTEMPTS {
        let suffix = TEMPFILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(".package-worker-{}-{suffix}.tmp", std::process::id());
        let path = parent.join(&name);
        #[cfg(windows)]
        let result = {
            let parent_handle =
                _parent_handle.ok_or_else(|| io::Error::other("missing parent handle"))?;
            create_relative_temp_file(parent_handle, std::ffi::OsStr::new(&name))
                .map(|file| (path.clone(), file))
        };
        #[cfg(not(windows))]
        let result = {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            options.open(&path).map(|file| (path.clone(), file))
        };
        match result {
            Ok(file) => return Ok(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create unique temporary file",
    )
    .into())
}

#[cfg(windows)]
fn create_relative_temp_file(parent: &File, name: &std::ffi::OsStr) -> io::Result<File> {
    use std::{
        os::windows::ffi::OsStrExt,
        os::windows::io::{AsRawHandle, FromRawHandle},
    };

    #[repr(C)]
    struct UnicodeString {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }
    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root_directory: *mut std::ffi::c_void,
        object_name: *mut UnicodeString,
        attributes: u32,
        security_descriptor: *mut std::ffi::c_void,
        security_quality_of_service: *mut std::ffi::c_void,
    }
    #[repr(C)]
    struct IoStatusBlock {
        status: i32,
        information: usize,
    }
    unsafe extern "system" {
        fn NtCreateFile(
            file_handle: *mut *mut std::ffi::c_void,
            desired_access: u32,
            object_attributes: *mut ObjectAttributes,
            io_status_block: *mut IoStatusBlock,
            allocation_size: *mut i64,
            file_attributes: u32,
            share_access: u32,
            create_disposition: u32,
            create_options: u32,
            ea_buffer: *mut std::ffi::c_void,
            ea_length: u32,
        ) -> i32;
    }

    let mut wide: Vec<u16> = name.encode_wide().collect();
    let byte_len = wide
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "temporary name too long"))?;
    if byte_len > u16::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "temporary name too long",
        ));
    }
    let mut unicode = UnicodeString {
        length: byte_len as u16,
        maximum_length: byte_len as u16,
        buffer: wide.as_mut_ptr(),
    };
    let mut attributes = ObjectAttributes {
        length: std::mem::size_of::<ObjectAttributes>() as u32,
        root_directory: parent.as_raw_handle(),
        object_name: &mut unicode,
        attributes: 0x00000040, // OBJ_CASE_INSENSITIVE
        security_descriptor: std::ptr::null_mut(),
        security_quality_of_service: std::ptr::null_mut(),
    };
    let mut status_block = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let mut handle = std::ptr::null_mut();
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            0x0013_0116, // generic write + DELETE for handle-relative cleanup
            &mut attributes,
            &mut status_block,
            std::ptr::null_mut(),
            0x00000080,  // FILE_ATTRIBUTE_NORMAL
            0x00000007,  // share read/write/delete
            0x00000002,  // FILE_CREATE
            0x0020_0060, // synchronous, non-directory, open reparse point
            std::ptr::null_mut(),
            0,
        )
    };
    if status == 0xC0000035u32 as i32 {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary file already exists",
        ));
    }
    if status < 0 {
        return Err(io::Error::other(format!(
            "NtCreateFile failed: 0x{status:08x}"
        )));
    }
    if handle.is_null() {
        return Err(io::Error::other("NtCreateFile returned a null handle"));
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}

#[cfg(windows)]
pub(super) fn delete_temp_file(temp: &File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    #[repr(C)]
    struct IoStatusBlock {
        status: i32,
        information: usize,
    }
    #[repr(C)]
    struct FileDispositionInformation {
        delete_file: u8,
    }
    unsafe extern "system" {
        fn NtSetInformationFile(
            file_handle: *mut std::ffi::c_void,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut std::ffi::c_void,
            length: u32,
            file_information_class: u32,
        ) -> i32;
    }
    let mut disposition = FileDispositionInformation { delete_file: 1 };
    let mut status_block = IoStatusBlock {
        status: 0,
        information: 0,
    };
    let status = unsafe {
        NtSetInformationFile(
            temp.as_raw_handle(),
            &mut status_block,
            (&mut disposition as *mut FileDispositionInformation).cast(),
            std::mem::size_of::<FileDispositionInformation>() as u32,
            13, // FileDispositionInformation
        )
    };
    if status < 0 {
        return Err(io::Error::other(format!(
            "NtSetInformationFile failed: 0x{status:08x}"
        )));
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn atomic_replace(
    temp: &File,
    parent: &File,
    target_name: &std::ffi::OsStr,
) -> io::Result<()> {
    use std::{
        os::windows::{ffi::OsStrExt, io::AsRawHandle},
        ptr,
    };
    use windows::Win32::Foundation::HANDLE;

    #[repr(C)]
    struct NtFileRenameInfo {
        replace_if_exists: u8,
        root_directory: HANDLE,
        file_name_length: u32,
        file_name: [u16; 1],
    }
    #[repr(C)]
    struct IoStatusBlock {
        status: i32,
        information: usize,
    }
    unsafe extern "system" {
        fn NtSetInformationFile(
            file_handle: HANDLE,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut std::ffi::c_void,
            length: u32,
            file_information_class: u32,
        ) -> i32;
    }

    let name: Vec<u16> = target_name.encode_wide().collect();
    if name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing target filename",
        ));
    }
    let size = std::mem::size_of::<NtFileRenameInfo>()
        .checked_add(
            name.len()
                .checked_mul(std::mem::size_of::<u16>())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "target filename too long")
                })?,
        )
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target filename too long"))?;
    let mut info = vec![0u8; size];
    unsafe {
        let rename = info.as_mut_ptr() as *mut NtFileRenameInfo;
        (*rename).replace_if_exists = 1;
        (*rename).root_directory = HANDLE(parent.as_raw_handle() as _);
        (*rename).file_name_length = (name.len() * std::mem::size_of::<u16>()) as u32;
        ptr::copy_nonoverlapping(name.as_ptr(), (*rename).file_name.as_mut_ptr(), name.len());
        let mut io_status = IoStatusBlock {
            status: 0,
            information: 0,
        };
        let status = NtSetInformationFile(
            HANDLE(temp.as_raw_handle() as _),
            &mut io_status,
            rename.cast(),
            info.len() as u32,
            10, // FileRenameInformation
        );
        if status < 0 {
            return Err(io::Error::other(format!(
                "NtSetInformationFile failed: 0x{status:08x}"
            )));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn atomic_replace(temp: &Path, target: &Path) -> io::Result<()> {
    fs::rename(temp, target)
}

#[cfg(windows)]
pub(super) fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
pub(super) fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}
