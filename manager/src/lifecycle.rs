//! What closing the window means, per OS.
//!
//! The animated dictation pill is a window of this process (`pill.rs`), so the
//! pill lives exactly as long as the Manager does. On macOS that is the whole
//! point of a menu-bar app: the window closes, the icon stays, dictation keeps
//! working. Windows has no menu-bar icon of its own — the tray belongs to the
//! Engine — so the same rule has to be spelled out: closing the window hides
//! it and the process stays behind with the pill. A second launch (the tray's
//! «Открыть интерфейс», a double click on the shortcut) finds the window the
//! first process still owns and shows it, so there is never a second pill.

use gpui::{App, Window};

/// Close request: the caption button, the app menu, the taskbar, Alt+F4.
pub fn request_close(window: &mut Window, cx: &mut App) {
    let _ = cx;
    #[cfg(target_os = "windows")]
    windows::hide(window);
    #[cfg(not(target_os = "windows"))]
    window.remove_window();
}

/// The platform asks before it takes the window away; a hidden window is what
/// keeps the pill alive, so the answer on Windows is always "no".
pub fn guard_close(window: &Window, cx: &App) {
    #[cfg(target_os = "windows")]
    {
        window.on_window_should_close(cx, |window, _cx| {
            windows::hide(window);
            false
        });
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (window, cx);
}

/// Remembers the window whose visibility the pill loop watches, so a hidden
/// window with no Engine behind it can leave instead of lingering unseen.
pub fn remember(window: &Window) {
    #[cfg(target_os = "windows")]
    windows::remember(window);
}

/// `false` only when the remembered window is known to be hidden.
pub fn main_visible() -> bool {
    #[cfg(target_os = "windows")]
    {
        return windows::main_visible();
    }
    #[cfg(not(target_os = "windows"))]
    true
}

/// True when another Manager already runs: it is shown and this process should
/// leave without touching the Engine.
pub fn reveal_existing(title: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        return windows::reveal(title);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = title;
        false
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::cell::Cell;
    use std::ffi::c_void;

    use gpui::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    thread_local! {
        /// The window the app opened; the pill loop reads its visibility from
        /// any thread, so it is stored as a raw handle rather than an entity.
        static MAIN: Cell<isize> = const { Cell::new(0) };
    }

    fn hwnd(window: &Window) -> Option<*mut c_void> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return None;
        };
        Some(win32.hwnd.get() as *mut c_void)
    }

    pub(super) fn remember(window: &Window) {
        if let Some(raw) = hwnd(window) {
            MAIN.with(|main| main.set(raw as isize));
        }
    }

    pub(super) fn hide(window: &Window) {
        if let Some(raw) = hwnd(window) {
            unsafe { ShowWindow(raw, SW_HIDE) };
        }
    }

    pub(super) fn main_visible() -> bool {
        let raw = MAIN.with(Cell::get);
        if raw == 0 {
            return true;
        }
        unsafe { IsWindowVisible(raw as *mut c_void) != 0 }
    }

    pub(super) fn reveal(title: &str) -> bool {
        let mut wide: Vec<u16> = title.encode_utf16().collect();
        wide.push(0);
        let found = unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) };
        if found.is_null() {
            return false;
        }
        unsafe {
            ShowWindow(found, SW_RESTORE);
            SetForegroundWindow(found);
        }
        true
    }

    const SW_HIDE: i32 = 0;
    const SW_RESTORE: i32 = 9;

    #[link(name = "user32")]
    extern "system" {
        fn ShowWindow(window: *mut c_void, command: i32) -> i32;
        fn IsWindowVisible(window: *mut c_void) -> i32;
        fn FindWindowW(class: *const u16, title: *const u16) -> *mut c_void;
        fn SetForegroundWindow(window: *mut c_void) -> i32;
    }
}
