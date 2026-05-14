use std::path::Path;
use std::time::Duration;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
};

use crate::{normalize_exe_path, tracked_app_id_for};

#[derive(Debug, Clone)]
pub struct ForegroundWindowSample {
    pub tracked_app_id: String,
    pub exe_path: String,
    pub normalized_exe_path: String,
    pub process_name: String,
    pub window_title: Option<String>,
    pub pid: u32,
    pub is_idle: bool,
}

pub fn capture_foreground_window(
    idle_threshold: Duration,
) -> Result<Option<ForegroundWindowSample>, String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return Ok(None);
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return Ok(None);
        }

        let exe_path = resolve_process_image_path(pid)?;
        if exe_path.trim().is_empty() {
            return Ok(None);
        }

        let normalized_exe_path = normalize_exe_path(&exe_path);
        let process_name = Path::new(&exe_path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        let window_title = read_window_title(hwnd);
        let idle_duration = current_idle_duration()?;

        Ok(Some(ForegroundWindowSample {
            tracked_app_id: tracked_app_id_for(&exe_path),
            exe_path,
            normalized_exe_path,
            process_name,
            window_title,
            pid,
            is_idle: idle_duration >= idle_threshold,
        }))
    }
}

fn resolve_process_image_path(pid: u32) -> Result<String, String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|error| error.to_string())?;

        let mut buffer = vec![0u16; 2048];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        result.map_err(|error| error.to_string())?;

        Ok(String::from_utf16_lossy(&buffer[..size as usize]))
    }
}

fn read_window_title(hwnd: HWND) -> Option<String> {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return None;
        }

        let mut buffer = vec![0u16; length as usize + 1];
        let written = GetWindowTextW(hwnd, &mut buffer);
        if written <= 0 {
            return None;
        }

        let title = String::from_utf16_lossy(&buffer[..written as usize]);
        let trimmed = title.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

fn current_idle_duration() -> Result<Duration, String> {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if !GetLastInputInfo(&mut info).as_bool() {
            return Err("GetLastInputInfo failed".to_string());
        }

        let now = GetTickCount64();
        Ok(Duration::from_millis(
            now.saturating_sub(info.dwTime as u64),
        ))
    }
}
