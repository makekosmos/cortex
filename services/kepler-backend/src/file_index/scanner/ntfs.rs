use super::{path_contains_noisy_folder, IndexedFile};
use std::collections::{HashMap, HashSet};
use std::mem::{size_of, MaybeUninit};
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{
    FSCTL_ENUM_USN_DATA, FSCTL_QUERY_USN_JOURNAL, MFT_ENUM_DATA_V0, USN_JOURNAL_DATA_V0,
    USN_RECORD_V2,
};
use windows::Win32::System::IO::DeviceIoControl;

const ENUM_BUFFER_LEN: usize = 1024 * 1024;
const NEXT_FRN_LEN: usize = size_of::<u64>();

#[derive(Clone)]
struct NtfsRecord {
    frn: u64,
    parent_frn: u64,
    name: String,
    is_dir: bool,
}

pub fn scan_drive_root(root: &Path, exclude_noisy: bool) -> Result<Vec<IndexedFile>, String> {
    let volume = volume_path(root)?;
    let handle = VolumeHandle::open(&volume)?;
    let journal = query_journal(handle.raw())?;
    let records = enum_records(handle.raw(), journal.NextUsn)?;
    Ok(records_to_files(root, records, exclude_noisy))
}

fn volume_path(root: &Path) -> Result<String, String> {
    let raw = root.to_string_lossy();
    let drive = raw
        .chars()
        .next()
        .filter(|letter| letter.is_ascii_alphabetic())
        .ok_or_else(|| format!("invalid drive root: {}", root.to_string_lossy()))?;
    Ok(format!(r"\\.\{}:", drive.to_ascii_uppercase()))
}

fn query_journal(handle: HANDLE) -> Result<USN_JOURNAL_DATA_V0, String> {
    let mut journal = MaybeUninit::<USN_JOURNAL_DATA_V0>::zeroed();
    let mut returned = 0;
    unsafe {
        DeviceIoControl(
            handle,
            FSCTL_QUERY_USN_JOURNAL,
            None,
            0,
            Some(journal.as_mut_ptr().cast()),
            size_of::<USN_JOURNAL_DATA_V0>() as u32,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|e| format!("query usn journal failed: {e}"))?;
    if returned < size_of::<USN_JOURNAL_DATA_V0>() as u32 {
        return Err("query usn journal returned a short buffer".to_string());
    }
    Ok(unsafe { journal.assume_init() })
}

fn enum_records(handle: HANDLE, high_usn: i64) -> Result<Vec<NtfsRecord>, String> {
    let mut input = MFT_ENUM_DATA_V0 {
        StartFileReferenceNumber: 0,
        LowUsn: 0,
        HighUsn: high_usn,
    };
    let mut buffer = vec![0u8; ENUM_BUFFER_LEN];
    let mut records = Vec::new();

    loop {
        let mut returned = 0;
        let result = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_ENUM_USN_DATA,
                Some((&input as *const MFT_ENUM_DATA_V0).cast()),
                size_of::<MFT_ENUM_DATA_V0>() as u32,
                Some(buffer.as_mut_ptr().cast()),
                buffer.len() as u32,
                Some(&mut returned),
                None,
            )
        };
        if let Err(error) = result {
            if !records.is_empty() {
                break;
            }
            return Err(format!("enum usn data failed: {error}"));
        }
        let returned = returned as usize;
        if returned <= NEXT_FRN_LEN {
            break;
        }
        input.StartFileReferenceNumber = u64::from_ne_bytes(
            buffer[..NEXT_FRN_LEN]
                .try_into()
                .map_err(|_| "missing next FRN")?,
        );
        read_records(&buffer[NEXT_FRN_LEN..returned], &mut records)?;
    }

    Ok(records)
}

fn read_records(bytes: &[u8], records: &mut Vec<NtfsRecord>) -> Result<(), String> {
    let mut offset = 0;
    while offset + size_of::<USN_RECORD_V2>() <= bytes.len() {
        let record =
            unsafe { std::ptr::read_unaligned(bytes[offset..].as_ptr().cast::<USN_RECORD_V2>()) };
        let record_len = record.RecordLength as usize;
        if record_len < size_of::<USN_RECORD_V2>() || offset + record_len > bytes.len() {
            return Err("usn record length exceeds enum buffer".to_string());
        }
        if record.MajorVersion == 2 {
            let name_start = offset + record.FileNameOffset as usize;
            let name_end = name_start + record.FileNameLength as usize;
            if name_end > offset + record_len || record.FileNameLength % 2 != 0 {
                return Err("usn filename exceeds record buffer".to_string());
            }
            let utf16 = bytes[name_start..name_end]
                .chunks_exact(2)
                .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
                .collect::<Vec<_>>();
            let name = String::from_utf16_lossy(&utf16);
            if !name.is_empty() {
                records.push(NtfsRecord {
                    frn: record.FileReferenceNumber,
                    parent_frn: record.ParentFileReferenceNumber,
                    name,
                    is_dir: record.FileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0,
                });
            }
        }
        offset += record_len;
    }
    Ok(())
}

fn records_to_files(
    root: &Path,
    records: Vec<NtfsRecord>,
    exclude_noisy: bool,
) -> Vec<IndexedFile> {
    let dirs = records
        .iter()
        .filter(|record| record.is_dir)
        .map(|record| (record.frn, record.clone()))
        .collect::<HashMap<_, _>>();
    let mut cache = HashMap::new();
    let mut out = Vec::new();

    for record in records.into_iter().filter(|record| !record.is_dir) {
        let mut visiting = HashSet::new();
        let Some(parent) =
            resolve_dir_path(record.parent_frn, root, &dirs, &mut cache, &mut visiting)
        else {
            continue;
        };
        let path = parent.join(&record.name);
        if exclude_noisy && path_contains_noisy_folder(&path) {
            continue;
        }
        out.push(IndexedFile {
            path: path.to_string_lossy().into_owned(),
            name: record.name,
            mtime: 0,
        });
    }

    out
}

fn resolve_dir_path(
    frn: u64,
    root: &Path,
    dirs: &HashMap<u64, NtfsRecord>,
    cache: &mut HashMap<u64, PathBuf>,
    visiting: &mut HashSet<u64>,
) -> Option<PathBuf> {
    if let Some(path) = cache.get(&frn) {
        return Some(path.clone());
    }
    if !visiting.insert(frn) {
        return None;
    }
    let record = dirs.get(&frn)?;
    let path = if record.parent_frn == record.frn || record.name == "." {
        root.to_path_buf()
    } else {
        resolve_dir_path(record.parent_frn, root, dirs, cache, visiting)?.join(&record.name)
    };
    cache.insert(frn, path.clone());
    Some(path)
}

struct VolumeHandle(HANDLE);

impl VolumeHandle {
    fn open(volume: &str) -> Result<Self, String> {
        let wide = volume
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                FILE_GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|e| format!("open volume {volume} failed: {e}"))?;
        Ok(Self(handle))
    }

    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for VolumeHandle {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(frn: u64, parent_frn: u64, name: &str) -> NtfsRecord {
        NtfsRecord {
            frn,
            parent_frn,
            name: name.to_string(),
            is_dir: true,
        }
    }

    #[test]
    fn resolves_file_paths_from_parent_records_and_filters_noisy_dirs() {
        let records = vec![
            dir(5, 5, "."),
            dir(6, 5, "Users"),
            dir(7, 6, "Kirill"),
            dir(8, 7, "node_modules"),
            NtfsRecord {
                frn: 9,
                parent_frn: 7,
                name: "note.md".to_string(),
                is_dir: false,
            },
            NtfsRecord {
                frn: 10,
                parent_frn: 8,
                name: "noise.js".to_string(),
                is_dir: false,
            },
        ];

        let files = records_to_files(Path::new(r"C:\"), records, true);

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, r"C:\Users\Kirill\note.md");
    }
}
