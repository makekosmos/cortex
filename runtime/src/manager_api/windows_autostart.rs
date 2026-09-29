//! Engine autostart lives in two HKCU values: the `Run` entry
//! (`"mundus-engine.exe" --start`) and the `StartupApproved` marker the OS
//! writes for Task Manager/Settings toggles. The Win32 registry API does
//! both — no `reg.exe` child process means nothing to hide a console
//! window for and no text output to parse.

use std::time::{SystemTime, UNIX_EPOCH};
use windows::core::PCWSTR;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_BINARY, REG_SZ,
    RRF_RT_ANY, RRF_RT_REG_BINARY,
};

const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED_SUBKEY: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn win32_status(status: WIN32_ERROR) -> Result<(), String> {
    if status == WIN32_ERROR(0) {
        Ok(())
    } else {
        Err(format!("registry write failed: {status:?}"))
    }
}

/// The 12-byte marker Windows writes for a Task Manager toggle: state
/// byte (2 = enabled, 3/6 = disabled) followed by a FILETIME. Writing
/// the same layout makes an `engine.autostart.set` opt-out
/// indistinguishable from an OS-level disable — and the installer's
/// migration honors both.
pub(super) fn approved_marker(disabled: bool) -> [u8; 12] {
    let mut marker = [0u8; 12];
    marker[0] = if disabled { 3 } else { 2 };
    let filetime = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .saturating_add(11_644_473_600))
    .saturating_mul(10_000_000);
    marker[4..].copy_from_slice(&filetime.to_le_bytes());
    marker
}

fn read_binary(subkey: &str, name: &str) -> Option<Vec<u8>> {
    let subkey = wide(subkey);
    let name = wide(name);
    unsafe {
        let mut size = 0u32;
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_BINARY,
            None,
            None,
            Some(&mut size),
        );
        if status != WIN32_ERROR(0) || size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_REG_BINARY,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut size),
        );
        (status == WIN32_ERROR(0)).then_some(data)
    }
}

fn value_present(subkey: &str, name: &str) -> bool {
    let subkey = wide(subkey);
    let name = wide(name);
    let mut size = 0u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            RRF_RT_ANY,
            None,
            None,
            Some(&mut size),
        )
    };
    status == WIN32_ERROR(0)
}

/// True while the entry is explicitly disabled in StartupApproved —
/// state byte 3 (and 6 on some builds for items disabled at sign-in);
/// 2 means enabled and absent means never toggled.
fn approved_disabled(name: &str) -> bool {
    read_binary(APPROVED_SUBKEY, name).is_some_and(|marker| matches!(marker.first(), Some(3 | 6)))
}

fn write_binary(subkey: &str, name: &str, data: &[u8]) -> Result<(), String> {
    let subkey = wide(subkey);
    let name = wide(name);
    win32_status(unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            REG_BINARY.0,
            Some(data.as_ptr().cast()),
            data.len() as u32,
        )
    })
}

fn write_sz(subkey: &str, name: &str, value: &str) -> Result<(), String> {
    let subkey = wide(subkey);
    let name = wide(name);
    let data = wide(value);
    win32_status(unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            (data.len() * 2) as u32,
        )
    })
}

fn delete_value(subkey: &str, name: &str) {
    let subkey = wide(subkey);
    let name = wide(name);
    unsafe {
        let _ = RegDeleteKeyValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(name.as_ptr()),
        );
    }
}

/// Enabled = Run value present AND no disabled StartupApproved marker —
/// an item the user disabled in Task Manager keeps its Run value but is
/// off.
pub(super) fn enabled() -> bool {
    value_present(RUN_SUBKEY, crate::brand::AUTOSTART_RUN_VALUE)
        && !approved_disabled(crate::brand::AUTOSTART_RUN_VALUE)
}

pub(super) fn set(enabled: bool) -> Result<(), String> {
    let name = crate::brand::AUTOSTART_RUN_VALUE;
    if enabled {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        write_sz(
            RUN_SUBKEY,
            name,
            &format!("\"{}\" --start", executable.display()),
        )?;
        // An enabled marker clears a stale disabled flag — the same
        // write Task Manager's Enable makes.
        write_binary(APPROVED_SUBKEY, name, &approved_marker(false))?;
    } else {
        // The opt-out marker lands first: if it cannot be written the
        // entry stays enabled and the caller reports the failure —
        // never silently claim an opt-out that was not persisted.
        write_binary(APPROVED_SUBKEY, name, &approved_marker(true))?;
        // The Run value stays: this is exactly Task Manager's Disable,
        // so the entry remains visible (and re-enableable) in the OS UI.
    }
    // MIGRATION(KOS-267): remove after 2026-11-01. A stale legacy Run
    // value would resurrect the old engine binary; drop the known old
    // names whenever the autostart preference is touched.
    for legacy in ["Kosmos Engine", "Kosmos"] {
        // MIGRATION(KOS-267)
        delete_value(RUN_SUBKEY, legacy);
    }
    Ok(())
}
