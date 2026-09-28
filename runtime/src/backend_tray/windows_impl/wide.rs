//! Small UTF-16 helper used by Win32 menu/icon APIs.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

pub fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
