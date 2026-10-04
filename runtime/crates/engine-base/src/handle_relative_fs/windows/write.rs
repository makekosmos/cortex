use super::open::{delete_handle, relative, relative_with, validate_opened};
use super::ops::relative_duplicate;
use super::{
    c_void, nt, Handle, Identity, IoStatusBlock, FILE_CREATE, FILE_OPEN, FILE_READ_ATTRIBUTES,
    SYNCHRONIZE,
};
use crate::handle_relative_fs::RootHandle;
use crate::handle_relative_fs::TEMP_FILE_SEQUENCE;
use std::{ffi::OsStr, io, sync::atomic::Ordering};

fn existing_identity(parent: Handle, name: &str) -> io::Result<Option<Identity>> {
    match relative_with(
        parent,
        name,
        false,
        FILE_READ_ATTRIBUTES | SYNCHRONIZE,
        FILE_OPEN,
    ) {
        Ok(file) => validate_opened(file.raw(), None, false).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn create_temp(parent: Handle, name: &str) -> io::Result<std::fs::File> {
    use std::os::windows::io::FromRawHandle;
    let temp = relative_with(parent, name, false, 0x0013_0116, FILE_CREATE)?;
    let raw = temp.0;
    std::mem::forget(temp);
    Ok(unsafe { std::fs::File::from_raw_handle(raw) })
}

fn atomic_replace(temp: Handle, parent: Handle, name: &str) -> io::Result<()> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    #[repr(C)]
    struct RenameInformation {
        replace_if_exists: u8,
        root_directory: Handle,
        file_name_length: u32,
        file_name: [u16; 1],
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
    let wide: Vec<u16> = OsStr::new(name).encode_wide().collect();
    if wide.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty filename",
        ));
    }
    let size = std::mem::size_of::<RenameInformation>()
        .checked_add(
            wide.len()
                .checked_mul(2)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "filename too long"))?,
        )
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "filename too long"))?;
    let mut buffer = vec![0u8; size];
    let rename = buffer.as_mut_ptr().cast::<RenameInformation>();
    unsafe {
        (*rename).replace_if_exists = 1;
        (*rename).root_directory = parent;
        (*rename).file_name_length = (wide.len() * 2) as u32;
        ptr::copy_nonoverlapping(wide.as_ptr(), (*rename).file_name.as_mut_ptr(), wide.len());
        let mut iosb = IoStatusBlock {
            status: 0,
            information: 0,
        };
        nt(NtSetInformationFile(
            temp,
            &mut iosb,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            10,
        ))
    }
}

pub(in crate::handle_relative_fs) fn write_relative(
    root: &RootHandle,
    components: &[&str],
    bytes: &[u8],
    max_bytes: usize,
) -> io::Result<()> {
    use std::{io::Write, os::windows::io::AsRawHandle};
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "file exceeds limit",
        ));
    }
    let mut parent = relative_duplicate(root.handle.raw())?;
    for component in &components[..components.len() - 1] {
        let child = relative(parent.raw(), component, true)?;
        validate_opened(child.raw(), None, true)?;
        parent = child;
    }
    let target = components[components.len() - 1];
    let before = existing_identity(parent.raw(), target)?;
    let mut temp = None;
    for _ in 0..128 {
        let name = format!(
            ".mundus-grant-{}-{}.tmp",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        match create_temp(parent.raw(), &name) {
            Ok(file) => {
                temp = Some((name, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    let (_temp_name, mut temp) = temp
        .ok_or_else(|| io::Error::new(io::ErrorKind::AlreadyExists, "temporary file collision"))?;
    let result = (|| {
        temp.write_all(bytes)?;
        temp.sync_all()?;
        let after = existing_identity(parent.raw(), target)?;
        if after != before {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "target identity changed",
            ));
        }
        atomic_replace(temp.as_raw_handle() as Handle, parent.raw(), target)
    })();
    if result.is_err() {
        let _ = delete_handle(temp.as_raw_handle() as Handle);
    }
    result
}
