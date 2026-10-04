// Windows source dispatch.

#![cfg(target_os = "windows")]

pub mod start_menu;
pub mod uwp;

use crate::app_index::app::{App, AppKind};
use crate::app_index::{AppIndexError, AppSource, Result};
use std::path::PathBuf;

/// Open a path or registered protocol through the Windows shell broker.
/// The target is passed as data to ShellExecuteW; no command interpreter is
/// involved.
pub fn shell_execute_open(target: &str) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let target: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR::null(),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if (result.0 as usize) <= 32 {
        return Err(AppIndexError::Launch(format!(
            "ShellExecuteW failed with code {}",
            result.0 as usize
        )));
    }
    Ok(())
}

pub fn sources(_icon_cache_dir: PathBuf) -> Vec<Box<dyn AppSource>> {
    vec![
        Box::new(start_menu::StartMenuSource),
        Box::new(uwp::UwpSource),
    ]
}

pub fn launch(app: &App) -> Result<()> {
    match app.kind {
        AppKind::Win32 => start_menu::launch_win32(&app.exec_path),
        AppKind::Uwp => uwp::launch_uwp(&app.exec_path),
        _ => Err(AppIndexError::Launch(format!(
            "unsupported AppKind on Windows: {:?}",
            app.kind
        ))),
    }
}
