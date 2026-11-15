//! What closing the window means, per OS.
//!
//! The Manager is only the interface: closing its window must not touch
//! dictation, which lives in the Engine, nor the pill, which is a plugin
//! process of its own. On macOS that is the whole point of a menu-bar app: the
//! window closes, the icon stays, dictation keeps working. Windows has no
//! menu-bar icon of its own — the tray belongs to the Engine — so the same rule
//! has to be spelled out: closing the window hides it and the process stays
//! behind, dictation unaffected. A second launch (the tray's «Открыть
//! интерфейс», a double click on the shortcut) finds the window the first
//! process still owns and shows it.

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
/// keeps the interface one key press away, so the answer on Windows is always
/// "no".
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
        /// The window the app opened, kept as a raw handle so it can be found
        /// again without an entity.
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
        fn FindWindowW(class: *const u16, title: *const u16) -> *mut c_void;
        fn SetForegroundWindow(window: *mut c_void) -> i32;
    }
}
