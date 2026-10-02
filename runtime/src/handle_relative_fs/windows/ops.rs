use super::enumerate::enumerate;
use super::open::{delete_handle, read_handle, relative, relative_with, validate_opened};
use super::{
    query, DuplicateHandle, FileStandardInfo, Handle, OwnedHandle, DELETE, FILE_ADD_SUBDIRECTORY,
    FILE_CREATE, FILE_LIST_DIRECTORY, FILE_OPEN, FILE_READ_ATTRIBUTES, FILE_STANDARD_INFO_CLASS,
    INVALID_HANDLE_VALUE, SYNCHRONIZE,
};
use crate::handle_relative_fs::{validate_components, RelativeEntry, RootHandle, RootIdentity};
use std::io;

pub(in crate::handle_relative_fs) fn read_relative(
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

pub(in crate::handle_relative_fs) fn list_relative(
    root: &RootHandle,
    components: &[&str],
    max_entries: usize,
) -> io::Result<Vec<RelativeEntry>> {
    if !components.is_empty() {
        validate_components(components)?;
    }
    let mut directory = relative_duplicate(root.handle.raw())?;
    for component in components {
        let child = relative(directory.raw(), component, true)?;
        validate_opened(child.raw(), None, true)?;
        directory = child;
    }
    let entries = enumerate(directory.raw(), max_entries)?;
    Ok(entries
        .into_iter()
        .map(|entry| RelativeEntry {
            name: entry.name,
            directory: entry.directory,
        })
        .collect())
}

pub(in crate::handle_relative_fs) fn delete_relative(
    root: &RootHandle,
    components: &[&str],
) -> io::Result<()> {
    let mut parent = relative_duplicate(root.handle.raw())?;
    for component in &components[..components.len() - 1] {
        let child = relative(parent.raw(), component, true)?;
        validate_opened(child.raw(), None, true)?;
        parent = child;
    }
    let target = components[components.len() - 1];
    let file = relative_with(
        parent.raw(),
        target,
        false,
        FILE_READ_ATTRIBUTES | DELETE | SYNCHRONIZE,
        FILE_OPEN,
    )?;
    validate_opened(file.raw(), None, false)?;
    delete_handle(file.raw())
}

pub(in crate::handle_relative_fs) fn mkdir_relative(
    root: &RootHandle,
    components: &[&str],
) -> io::Result<()> {
    let mut parent = relative_duplicate(root.handle.raw())?;
    for component in components {
        let child = match relative_with(
            parent.raw(),
            component,
            true,
            FILE_ADD_SUBDIRECTORY | FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            FILE_CREATE,
        ) {
            Ok(child) => child,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                relative(parent.raw(), component, true)?
            }
            Err(error) => return Err(error),
        };
        validate_opened(child.raw(), None, true)?;
        parent = child;
    }
    Ok(())
}

pub(in crate::handle_relative_fs) fn stat_relative(
    root: &RootHandle,
    components: &[&str],
) -> io::Result<u64> {
    let mut parent = relative_duplicate(root.handle.raw())?;
    for component in &components[..components.len() - 1] {
        let child = relative(parent.raw(), component, true)?;
        validate_opened(child.raw(), None, true)?;
        parent = child;
    }
    let file = relative(parent.raw(), components[components.len() - 1], false)?;
    validate_opened(file.raw(), None, false)?;
    let standard: FileStandardInfo = query(file.raw(), FILE_STANDARD_INFO_CLASS)?;
    if standard.end_of_file < 0 {
        return Err(io::Error::other("negative file size"));
    }
    Ok(standard.end_of_file as u64)
}

pub(in crate::handle_relative_fs) fn file_identity(
    root: &RootHandle,
    component: &str,
) -> io::Result<RootIdentity> {
    let file = relative(root.handle.raw(), component, false)?;
    let identity = validate_opened(file.raw(), None, false)?;
    let mut secondary = [0u8; 8];
    secondary.copy_from_slice(&identity.file_id_bytes()[..8]);
    Ok(RootIdentity {
        primary: identity.volume_serial(),
        secondary: u64::from_le_bytes(secondary),
    })
}
pub(super) fn relative_duplicate(handle: Handle) -> io::Result<OwnedHandle> {
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
