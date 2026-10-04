#[cfg(windows)]
fn hold_executable(path: &Path) -> Result<fs::File, WorkerProcessError> {
    use std::os::windows::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|_| WorkerProcessError::InvalidExecutable)
}

fn validate_executable(path: &Path) -> Result<(), WorkerProcessError> {
    #[cfg(target_os = "macos")]
    return validate_macos_executable(path);
    #[cfg(not(target_os = "macos"))]
    {
        if !path.is_absolute()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            || !path.is_file()
        {
            return Err(WorkerProcessError::InvalidExecutable);
        }
        if is_package_entrypoint(path)
            && PackageStore::verify_immutable_entrypoint_path(path).is_err()
        {
            return Err(WorkerProcessError::InvalidExecutable);
        }
        if !is_windows_pe(path) {
            return Err(WorkerProcessError::InvalidExecutable);
        }
        Ok(())
    }
}

fn is_package_entrypoint(path: &Path) -> bool {
    path.ancestors().any(|candidate| {
        candidate
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_hash)
            && candidate
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|name| name == "unpacked")
    })
}

fn is_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_windows_pe(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() < 64 {
        return false;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut dos = [0u8; 64];
    if file.read_exact(&mut dos).is_err() || &dos[..2] != b"MZ" {
        return false;
    }
    let pe_offset = u32::from_le_bytes([dos[0x3c], dos[0x3d], dos[0x3e], dos[0x3f]]) as u64;
    if pe_offset.checked_add(4).is_none() || pe_offset + 4 > metadata.len() {
        return false;
    }
    file.seek(SeekFrom::Start(pe_offset)).is_ok()
        && file.read_exact(&mut dos[..4]).is_ok()
        && &dos[..4] == b"PE\0\0"
}
