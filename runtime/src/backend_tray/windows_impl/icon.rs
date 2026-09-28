//! Tray icon discovery and Win32 NOTIFYICONDATA construction.

use std::env;
use std::path::{Path, PathBuf};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    LoadImageW, HICON, IMAGE_ICON, LR_DEFAULTSIZE, LR_LOADFROMFILE,
};

use super::wide::wide;

pub const WM_TRAY_CALLBACK: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 1;

pub fn resolve_icon_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("MUNDUS_TRAY_ICON") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("tray.ico"));
            if let Some(app_dir) = parent.parent() {
                candidates.push(app_dir.join("tray.ico"));
            }
        }
    }
    for variable in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env::var_os(variable) {
            candidates.push(PathBuf::from(root).join("Mundus/tray.ico"));
        }
    }
    candidates
        .into_iter()
        .find(|path| path.is_file() && path.extension() == Some(std::ffi::OsStr::new("ico")))
}

pub fn load_icon(path: &Path) -> Option<HICON> {
    let path = wide(path.as_os_str());
    unsafe {
        LoadImageW(
            HINSTANCE::default(),
            PCWSTR(path.as_ptr()),
            IMAGE_ICON,
            0,
            0,
            LR_LOADFROMFILE | LR_DEFAULTSIZE,
        )
        .ok()
        .map(|handle| HICON(handle.0))
    }
}

pub fn notify_data(window: HWND, icon: HICON) -> NOTIFYICONDATAW {
    let tip = wide(engine::brand::PRODUCT_NAME);
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: window,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY_CALLBACK,
        hIcon: icon,
        ..Default::default()
    };
    let length = tip.len().saturating_sub(1).min(data.szTip.len() - 1);
    data.szTip[..length].copy_from_slice(&tip[..length]);
    data
}

pub fn remove_tray_icon(notify: &NOTIFYICONDATAW) {
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, notify);
    }
}
