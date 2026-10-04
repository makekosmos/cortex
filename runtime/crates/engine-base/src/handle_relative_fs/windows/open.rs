use super::*;

pub(super) fn validate_opened(
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
        if expected.volume_serial != id.volume_serial || expected.file_id.bytes != id.file_id.bytes
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
pub(in crate::handle_relative_fs) fn open_root(path: &Path) -> io::Result<RootHandle> {
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
pub(super) fn relative_with(
    parent: Handle,
    name: &str,
    directory: bool,
    access: u32,
    disposition: u32,
) -> io::Result<OwnedHandle> {
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
            access,
            &mut oa,
            &mut iosb,
            std::ptr::null_mut(),
            0,
            FILE_SHARE_ALL,
            disposition,
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
    if status == 0xC0000035u32 as i32 {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "name already exists",
        ));
    }
    if status == 0xC0000034u32 as i32 || status == 0xC000003Au32 as i32 {
        return Err(io::Error::new(io::ErrorKind::NotFound, "name not found"));
    }
    nt(status)?;
    if owned.raw().is_null() {
        return Err(io::Error::other("null handle"));
    }
    Ok(owned)
}
pub(super) fn relative(parent: Handle, name: &str, directory: bool) -> io::Result<OwnedHandle> {
    relative_with(
        parent,
        name,
        directory,
        if directory {
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE
        } else {
            FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE
        },
        FILE_OPEN,
    )
}

pub(super) fn delete_handle(handle: Handle) -> io::Result<()> {
    #[repr(C)]
    struct FileDispositionInformation {
        delete_file: u8,
    }
    unsafe extern "system" {
        fn NtSetInformationFile(
            file_handle: Handle,
            io_status_block: *mut IoStatusBlock,
            file_information: *mut c_void,
            length: u32,
            file_information_class: u32,
        ) -> i32;
    }
    let mut disposition = FileDispositionInformation { delete_file: 1 };
    let mut iosb = IoStatusBlock {
        status: 0,
        information: 0,
    };
    nt(unsafe {
        NtSetInformationFile(
            handle,
            &mut iosb,
            (&mut disposition as *mut FileDispositionInformation).cast(),
            std::mem::size_of::<FileDispositionInformation>() as u32,
            13,
        )
    })
}

pub(super) fn read_handle(handle: Handle, max: usize) -> io::Result<Vec<u8>> {
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
