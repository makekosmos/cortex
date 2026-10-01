//! Human-readable app name from the exe version resource (FileDescription,
//! then ProductName, then the file stem). The usage tracker stamps it into
//! `tracked_apps.display_name` and the `app_index.exe_info` op serves it for
//! legacy rows — a browser must report "Microsoft Edge", never the last tab
//! title (KOS-287).
//!
//! Sync FFI; callers run it on a blocking-pool thread.

/// Best-effort display name for `exe_path`. `None` when the path is empty or
/// the file is gone (a stale path from a removed version dir must not error —
/// the row just keeps its stored name).
pub fn exe_display_name(exe_path: &str) -> Option<String> {
    if exe_path.trim().is_empty() || !std::path::Path::new(exe_path).is_file() {
        return None;
    }
    version_string(exe_path, "FileDescription")
        .or_else(|| version_string(exe_path, "ProductName"))
        .or_else(|| file_stem(exe_path))
}

fn file_stem(exe_path: &str) -> Option<String> {
    std::path::Path::new(exe_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(target_os = "windows")]
fn version_string(exe_path: &str, name: &str) -> Option<String> {
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };

    unsafe {
        let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut reserved = 0u32;
        let size =
            GetFileVersionInfoSizeW(windows::core::PCWSTR(wide.as_ptr()), Some(&mut reserved));
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(
            windows::core::PCWSTR(wide.as_ptr()),
            0,
            size,
            data.as_mut_ptr().cast(),
        )
        .is_err()
        {
            return None;
        }

        let mut value: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut value_len = 0u32;
        // "\VarFileInfo\Translation" yields packed lang|codepage u32s; the
        // StringFileInfo sub-block names are built from them. Fallback pairs
        // cover exes that ship only en-US or a neutral block.
        let mut translations: Vec<String> = Vec::new();
        if VerQueryValueW(
            data.as_ptr().cast(),
            windows::core::w!("\\VarFileInfo\\Translation"),
            &mut value,
            &mut value_len,
        )
        .as_bool()
            && !value.is_null()
            && value_len >= 4
        {
            let pairs = std::slice::from_raw_parts(value.cast::<u32>(), (value_len / 4) as usize);
            for pair in pairs {
                let lang = pair & 0xFFFF;
                let codepage = pair >> 16;
                translations.push(format!("{lang:04x}{codepage:04x}"));
            }
        }
        for extra in ["040904b0", "040904e4", "000004b0"] {
            if !translations.iter().any(|t| t == extra) {
                translations.push(extra.to_string());
            }
        }

        for translation in translations {
            let sub_block = format!("\\StringFileInfo\\{translation}\\{name}\0")
                .encode_utf16()
                .collect::<Vec<u16>>();
            let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut len = 0u32;
            if VerQueryValueW(
                data.as_ptr().cast(),
                windows::core::PCWSTR(sub_block.as_ptr()),
                &mut ptr,
                &mut len,
            )
            .as_bool()
                && !ptr.is_null()
                && len > 0
            {
                let text = String::from_utf16_lossy(std::slice::from_raw_parts(
                    ptr.cast::<u16>(),
                    len as usize,
                ));
                let text = text.trim_end_matches('\0').trim();
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            }
        }
        None
    }
}

#[cfg(not(target_os = "windows"))]
fn version_string(_exe_path: &str, _name: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_empty_paths_yield_none() {
        assert_eq!(exe_display_name(""), None);
        assert_eq!(
            exe_display_name("c:\\definitely\\missing\\app-1.2.3\\gone.exe"),
            None
        );
    }

    #[test]
    fn file_stem_is_the_last_resort() {
        assert_eq!(
            file_stem("c:\\apps\\discord\\app-1.0.2\\Discord.exe"),
            Some("Discord".to_string())
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn reads_version_info_of_a_real_exe() {
        // The test binary itself may lack a version resource; a system exe
        // always has one. Whatever it returns must be non-empty text.
        let name = exe_display_name(r"C:\Windows\explorer.exe");
        assert_eq!(
            name.as_deref().map(str::is_empty),
            Some(false),
            "explorer.exe must yield a display name"
        );
    }
}
