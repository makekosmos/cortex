use super::super::{directory_record::parse_directory_record, validate_components};
use super::*;

#[repr(C)]
pub(super) struct Entry {
    pub(super) name: String,
    pub(super) directory: bool,
    pub(super) reparse: bool,
    pub(super) id: FileId128,
}

pub(super) fn enumerate(dir: Handle, max_entries: usize) -> io::Result<Vec<Entry>> {
    enumerate_impl(dir, max_entries, false)
}
pub(super) fn enumerate_impl(
    dir: Handle,
    max_entries: usize,
    keep_reparse: bool,
) -> io::Result<Vec<Entry>> {
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
            if parsed.reparse_tag != 0 && !keep_reparse {
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
                    reparse: parsed.reparse_tag != 0,
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
