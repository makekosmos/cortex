use super::{TrayEvent, UnboundedSender};
use std::env;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::SyncSender;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, LoadImageW, PeekMessageW,
    PostQuitMessage, PostThreadMessageW, RegisterClassW, SetForegroundWindow, TrackPopupMenu,
    TranslateMessage, HICON, IMAGE_ICON, LR_DEFAULTSIZE, LR_LOADFROMFILE, MF_SEPARATOR, MF_STRING,
    MSG, PM_NOREMOVE, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_DESTROY, WM_LBUTTONDBLCLK, WM_QUIT,
    WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};

const WM_TRAY_CALLBACK: u32 = WM_USER + 1;
const MENU_OPEN: usize = 1;
const MENU_EXIT: usize = 2;

static EVENTS: std::sync::OnceLock<UnboundedSender<TrayEvent>> = std::sync::OnceLock::new();

pub fn run(events: UnboundedSender<TrayEvent>, ready: SyncSender<u32>) {
    let thread_id = unsafe { GetCurrentThreadId() };
    let mut bootstrap = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut bootstrap, HWND::default(), 0, 0, PM_NOREMOVE);
    }
    let _ = ready.send(thread_id);
    let _ = EVENTS.set(events);

    let Ok(module) = (unsafe { GetModuleHandleW(None) }) else {
        eprintln!("[kepler-backend] tray: GetModuleHandleW failed");
        return;
    };
    let instance = HINSTANCE(module.0);
    let class_name = wide("KosmosBackendTray");
    let class = WNDCLASSW {
        hInstance: instance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        lpfnWndProc: Some(window_proc),
        ..Default::default()
    };
    unsafe {
        let _ = RegisterClassW(&class);
    }

    let icon_path = match resolve_icon_path() {
        Some(path) => path,
        None => {
            eprintln!("[kepler-backend] tray icon asset not found");
            return;
        }
    };
    let icon = match load_icon(&icon_path) {
        Some(icon) => icon,
        None => {
            eprintln!(
                "[kepler-backend] tray icon failed to load: {}",
                icon_path.display()
            );
            return;
        }
    };
    let Ok(window) = (unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(class_name.as_ptr()),
            WS_POPUP,
            0,
            0,
            0,
            0,
            HWND::default(),
            None,
            instance,
            None,
        )
    }) else {
        eprintln!("[kepler-backend] tray window creation failed");
        unsafe {
            let _ = DestroyIcon(icon);
        }
        return;
    };

    let mut notify = notify_data(window, icon);
    unsafe {
        if !Shell_NotifyIconW(NIM_ADD, &mut notify).as_bool() {
            eprintln!("[kepler-backend] Shell_NotifyIconW(NIM_ADD) failed");
            let _ = DestroyWindow(window);
            let _ = DestroyIcon(icon);
            return;
        }
    }
    eprintln!("[kepler-backend] tray created");
    tracing::info!(target: "tray", "tray created");

    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, HWND::default(), 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = Shell_NotifyIconW(NIM_DELETE, &mut notify);
        let _ = DestroyWindow(window);
        let _ = DestroyIcon(icon);
    }
}

pub fn stop(thread_id: u32) {
    if thread_id != 0 {
        unsafe {
            let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

fn notify_data(window: HWND, icon: HICON) -> NOTIFYICONDATAW {
    let tip = wide("Kosmos Runtime");
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

fn load_icon(path: &Path) -> Option<HICON> {
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

fn resolve_icon_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("KOSMOS_TRAY_ICON") {
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
            candidates.push(PathBuf::from(root).join("Kosmos/tray.ico"));
        }
    }
    candidates
        .into_iter()
        .find(|path| path.is_file() && path.extension() == Some(OsStr::new("ico")))
}

fn resolve_cortex_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("KOSMOS_CORTEX_EXECUTABLE") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("Kosmos.exe"));
            if let Some(app_dir) = parent.parent() {
                candidates.push(app_dir.join("Kosmos.exe"));
            }
        }
    }
    for variable in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = env::var_os(variable) {
            candidates.push(PathBuf::from(root.clone()).join("Programs/Kosmos/Kosmos.exe"));
            candidates.push(PathBuf::from(root).join("Kosmos/Kosmos.exe"));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn open_cortex() {
    if let Some(executable) = resolve_cortex_executable() {
        if let Err(error) = Command::new(&executable).spawn() {
            eprintln!(
                "[kepler-backend] failed to open Cortex {}: {error}",
                executable.display()
            );
        }
    }
}

fn show_context_menu(window: HWND) {
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    let open = wide("Открыть");
    let exit = wide("Выход");
    unsafe {
        if resolve_cortex_executable().is_some() {
            let _ = AppendMenuW(menu, MF_STRING, MENU_OPEN, PCWSTR(open.as_ptr()));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        }
        let _ = AppendMenuW(menu, MF_STRING, MENU_EXIT, PCWSTR(exit.as_ptr()));
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(window);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            window,
            None,
        )
        .0 as usize;
        let _ = DestroyMenu(menu);
        match command {
            MENU_OPEN => open_cortex(),
            MENU_EXIT => {
                if let Some(events) = EVENTS.get() {
                    let _ = events.send(TrayEvent::Exit);
                }
            }
            _ => {}
        }
    }
}

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_TRAY_CALLBACK => match lparam.0 as u32 {
            WM_RBUTTONUP => show_context_menu(window),
            WM_LBUTTONDBLCLK => open_cortex(),
            _ => {}
        },
        WM_DESTROY => {
            PostQuitMessage(0);
        }
        _ => return DefWindowProcW(window, message, wparam, lparam),
    }
    LRESULT(0)
}
