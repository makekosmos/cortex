use super::{TrayEvent, UnboundedSender};
use std::process::Command;
use std::sync::mpsc::SyncSender;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIM_ADD};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
    DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, PeekMessageW, PostQuitMessage,
    PostThreadMessageW, RegisterClassW, SetForegroundWindow, TrackPopupMenu, TranslateMessage,
    MF_SEPARATOR, MF_STRING, MSG, PM_NOREMOVE, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_DESTROY,
    WM_LBUTTONDBLCLK, WM_QUIT, WM_RBUTTONUP, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};

mod components;
mod icon;
mod menu;
mod paths;
mod resolve;
mod wide;

use components::Component;
use icon::{load_icon, notify_data, remove_tray_icon, resolve_icon_path, WM_TRAY_CALLBACK};
use menu::{build_menu, MenuAction, MenuPresence};
use resolve::resolve_component_executable;
use wide::wide;

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

    let notify = notify_data(window, icon);
    unsafe {
        if !Shell_NotifyIconW(NIM_ADD, &notify).as_bool() {
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
        remove_tray_icon(&notify);
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

/// Launches a packaged GPUI component (Manager/Agenda/Memoria/Dictation).
fn open_component(component: Component) {
    if let Some(executable) = resolve_component_executable(component) {
        if let Err(error) = Command::new(&executable).spawn() {
            eprintln!(
                "[kepler-backend] failed to open {} {}: {error}",
                component.menu_label(),
                executable.display()
            );
        }
    }
}

fn current_presence() -> MenuPresence {
    MenuPresence {
        manager: resolve_component_executable(Component::Manager).is_some(),
        agenda: resolve_component_executable(Component::Agenda).is_some(),
        memoria: resolve_component_executable(Component::Memoria).is_some(),
        dictation: resolve_component_executable(Component::Dictation).is_some(),
    }
}

fn show_context_menu(window: HWND) {
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    let entries = build_menu(current_presence());
    let labels: Vec<Vec<u16>> = entries
        .iter()
        .map(|entry| wide(entry.action.label()))
        .collect();
    unsafe {
        for (entry, label) in entries.iter().zip(labels.iter()) {
            if entry.separator_before {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            }
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                entry.action.command_id(),
                PCWSTR(label.as_ptr()),
            );
        }
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
        match MenuAction::from_command_id(command) {
            Some(MenuAction::OpenManager) => open_component(Component::Manager),
            Some(MenuAction::OpenComponent(component)) => open_component(component),
            Some(MenuAction::Exit) => {
                if let Some(events) = EVENTS.get() {
                    let _ = events.send(TrayEvent::Exit);
                }
            }
            None => {}
        }
    }
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
            WM_LBUTTONDBLCLK => open_component(Component::Manager),
            _ => {}
        },
        WM_DESTROY => {
            PostQuitMessage(0);
        }
        _ => return DefWindowProcW(window, message, wparam, lparam),
    }
    LRESULT(0)
}
