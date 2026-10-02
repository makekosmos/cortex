//! Handle-relative filesystem access for package snapshots and grants.
use std::{io, sync::atomic::AtomicU64};

mod directory_record;
mod ops;
#[cfg(test)]
mod tests;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
mod unix_walk;
mod walk;
#[cfg(windows)]
mod windows;

pub use ops::{
    delete_relative, file_identity, list_relative, mkdir_relative, open_root, read_relative,
    root_identity, stat_relative, write_relative,
};
pub use walk::{walk_entries, walk_files};

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeEntry {
    pub name: String,
    pub directory: bool,
}

/// One regular file found by [`walk_entries`] — components only, no bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub components: Vec<String>,
    pub size: u64,
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

pub(crate) fn validate_components(components: &[&str]) -> io::Result<()> {
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
