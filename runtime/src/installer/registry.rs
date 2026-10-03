//! Minimal HKCU registry access for the installer subcommands (KOS-306).
//!
//! Everything the installer used to do through `Get-ItemProperty` /
//! `Set-ItemProperty` / `New-ItemProperty` in PowerShell goes through the
//! Win32 registry API here — no `reg.exe` child process, no text parsing.
//! All functions take the subkey relative to `HKEY_CURRENT_USER`; a leading
//! `HKCU:`/`HKCU\` prefix is accepted so tests can pass scratch paths in the
//! same shape the old PowerShell scripts did.

/// `HKCU\...\Run` — Engine autostart entry lives here.
pub(crate) const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// `HKCU\...\StartupApproved\Run` — the Task Manager enable/disable markers.
pub(crate) const APPROVED_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

/// A raw registry value: the REG_* type plus the raw data bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegValue {
    pub kind: u32,
    pub data: Vec<u8>,
}

/// Strip a leading `HKCU:`/`HKCU\`/`HKEY_CURRENT_USER\` prefix; everything
/// this module touches lives under the current user's hive.
pub(crate) fn hkcu_subkey(path: &str) -> &str {
    for prefix in ["HKCU:\\", "HKCU:", r"HKCU\", r"HKEY_CURRENT_USER\"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            return rest.trim_start_matches('\\');
        }
    }
    path
}

#[cfg(windows)]
mod imp {
    use super::RegValue;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegDeleteTreeW, RegGetValueW,
        RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE,
        REG_OPEN_CREATE_OPTIONS, REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE, RRF_RT_ANY,
    };

    pub(super) fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn win32(status: WIN32_ERROR, what: &str) -> Result<(), String> {
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("{what}: {status:?}"))
        }
    }

    fn open(subkey: &str, access: REG_SAM_FLAGS) -> Result<Option<HKEY>, String> {
        let wide_key = wide(subkey);
        let mut handle = HKEY::default();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(wide_key.as_ptr()),
                0,
                access,
                &mut handle,
            )
        };
        match status {
            ERROR_SUCCESS => Ok(Some(handle)),
            ERROR_FILE_NOT_FOUND => Ok(None),
            other => win32(other, &format!("RegOpenKeyExW {subkey}")).map(|_| None),
        }
    }

    pub fn key_exists(subkey: &str) -> Result<bool, String> {
        Ok(open(subkey, KEY_READ)?.is_some())
    }

    pub fn create_key(subkey: &str) -> Result<(), String> {
        let wide_key = wide(subkey);
        let mut handle = HKEY::default();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(wide_key.as_ptr()),
                0,
                PCWSTR::null(),
                REG_OPEN_CREATE_OPTIONS(0),
                KEY_WRITE,
                None,
                &mut handle,
                None,
            )
        };
        if status == ERROR_SUCCESS {
            unsafe {
                let _ = RegCloseKey(handle);
            }
        }
        win32(status, &format!("RegCreateKeyExW {subkey}"))
    }

    pub fn delete_tree(subkey: &str) -> Result<(), String> {
        let wide_key = wide(subkey);
        win32(
            unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wide_key.as_ptr())) },
            &format!("RegDeleteTreeW {subkey}"),
        )
    }

    pub fn read(subkey: &str, name: &str) -> Result<Option<RegValue>, String> {
        let wide_key = wide(subkey);
        let wide_name = wide(name);
        unsafe {
            let mut kind = REG_VALUE_TYPE(0);
            let mut size = 0u32;
            let status = RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(wide_key.as_ptr()),
                PCWSTR(wide_name.as_ptr()),
                RRF_RT_ANY,
                Some(&mut kind),
                None,
                Some(&mut size),
            );
            if status != ERROR_SUCCESS {
                return Ok(None);
            }
            let mut data = vec![0u8; size as usize];
            let status = RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(wide_key.as_ptr()),
                PCWSTR(wide_name.as_ptr()),
                RRF_RT_ANY,
                Some(&mut kind),
                Some(data.as_mut_ptr().cast()),
                Some(&mut size),
            );
            if status != ERROR_SUCCESS {
                return Ok(None);
            }
            Ok(Some(RegValue { kind: kind.0, data }))
        }
    }

    pub fn read_sz(subkey: &str, name: &str) -> Option<String> {
        let value = read(subkey, name).ok()??;
        if value.kind != REG_SZ.0 {
            return None;
        }
        let units: Vec<u16> = value
            .data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        Some(
            String::from_utf16_lossy(&units)
                .trim_end_matches('\0')
                .to_owned(),
        )
    }

    pub fn write(subkey: &str, name: &str, value: &RegValue) -> Result<(), String> {
        create_key(subkey)?;
        let Some(handle) = open(subkey, KEY_WRITE)? else {
            return Err(format!("RegOpenKeyExW {subkey}: key vanished after create"));
        };
        let wide_name = wide(name);
        let status = unsafe {
            let status = RegSetValueExW(
                handle,
                PCWSTR(wide_name.as_ptr()),
                0,
                REG_VALUE_TYPE(value.kind),
                Some(&value.data),
            );
            let _ = RegCloseKey(handle);
            status
        };
        win32(status, &format!("RegSetValueExW {subkey}\\{name}"))
    }

    pub fn write_sz(subkey: &str, name: &str, text: &str) -> Result<(), String> {
        let data: Vec<u8> = wide(text).iter().flat_map(|u| u.to_le_bytes()).collect();
        write(
            subkey,
            name,
            &RegValue {
                kind: REG_SZ.0,
                data,
            },
        )
    }

    pub fn write_binary(subkey: &str, name: &str, bytes: &[u8]) -> Result<(), String> {
        write(
            subkey,
            name,
            &RegValue {
                kind: windows::Win32::System::Registry::REG_BINARY.0,
                data: bytes.to_vec(),
            },
        )
    }

    pub fn delete_value(subkey: &str, name: &str) {
        let wide_key = wide(subkey);
        let wide_name = wide(name);
        unsafe {
            let _ = RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                PCWSTR(wide_key.as_ptr()),
                PCWSTR(wide_name.as_ptr()),
            );
        }
    }
}

// No non-Windows stub: every caller (autostart, legacy migration, tests) is
// already `cfg(windows)`-gated — registry writes are a Windows-only
// operation, and a stub that answers "Windows-only" at runtime would only
// mask a call site that forgot the gate.
#[cfg(windows)]
pub(crate) use imp::*;
