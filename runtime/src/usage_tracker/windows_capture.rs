// Win32 active-window sampler. Portированно из services/usage-tracker (Phase E1
// refactor). Sync-API: caller вызывает `capture_foreground_window` из контекста
// `tokio::task::spawn_blocking`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};

use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HWND, LPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible,
};

pub const PLATFORM: &str = "windows";

#[derive(Debug, Clone)]
pub struct ForegroundWindowSample {
    pub tracked_app_id: String,
    pub exe_path: String,
    pub normalized_exe_path: String,
    pub process_name: String,
    pub window_title: Option<String>,
    pub pid: u32,
    pub is_idle: bool,
    pub is_private: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessWindowState {
    Dead,
    AliveHidden,
    AliveVisible,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessProbeDiagnostics {
    pub enum_windows_calls: u32,
    pub process_path_queries: u32,
    pub visible_window_count: usize,
}

#[derive(Debug, Default)]
pub struct WindowSnapshot {
    pub visible_pids: HashSet<u32>,
    pub pid_to_has_visible_window: HashMap<u32, bool>,
}

impl WindowSnapshot {
    pub fn capture(diagnostics: &mut ProcessProbeDiagnostics) -> Self {
        #[derive(Debug)]
        struct EnumState {
            visible_pids: HashSet<u32>,
            visible_window_count: usize,
        }

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let state = &mut *(lparam.0 as *mut EnumState);
            if IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() {
                let mut window_pid = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
                if window_pid != 0 {
                    state.visible_pids.insert(window_pid);
                    state.visible_window_count += 1;
                }
            }
            BOOL(1)
        }

        let mut state = EnumState {
            visible_pids: HashSet::new(),
            visible_window_count: 0,
        };
        unsafe {
            let _ = EnumWindows(
                Some(enum_proc),
                LPARAM((&mut state as *mut EnumState) as isize),
            );
        }
        diagnostics.enum_windows_calls = diagnostics.enum_windows_calls.saturating_add(1);
        diagnostics.visible_window_count = state.visible_window_count;

        Self {
            visible_pids: state.visible_pids,
            pid_to_has_visible_window: HashMap::new(),
        }
    }

    pub fn has_visible_window(&mut self, pid: u32) -> bool {
        if let Some(value) = self.pid_to_has_visible_window.get(&pid) {
            return *value;
        }
        let value = self.visible_pids.contains(&pid);
        self.pid_to_has_visible_window.insert(pid, value);
        value
    }
}

#[derive(Debug, Clone)]
struct CachedProcessInfo {
    normalized_exe_path: String,
    creation_time_100ns: u64,
}

#[derive(Debug, Default)]
pub struct ProcessProbeCache {
    entries: HashMap<u32, CachedProcessInfo>,
}

pub fn capture_foreground_window_with_diagnostics(
    idle_threshold: Duration,
    diagnostics: &mut ProcessProbeDiagnostics,
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

        let process_info = query_process_info(pid, true, diagnostics)?;
        let Some(exe_path) = process_info.exe_path else {
            return Ok(None);
        };
        if exe_path.trim().is_empty() {
            return Ok(None);
        }

        let normalized_exe_path = normalize_exe_path(&exe_path);
        let process_name = Path::new(&exe_path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_string();
        let native_title = read_window_title(hwnd);
        let (window_title, is_private) =
            privacy_safe_window_title(hwnd, &process_name, native_title);
        let idle_duration = current_idle_duration()?;

        Ok(Some(ForegroundWindowSample {
            tracked_app_id: tracked_app_id_for(&exe_path),
            exe_path,
            normalized_exe_path,
            process_name,
            window_title,
            pid,
            is_idle: idle_duration >= idle_threshold,
            is_private,
        }))
    }
}

fn query_process_info(
    pid: u32,
    include_image_path: bool,
    diagnostics: &mut ProcessProbeDiagnostics,
) -> Result<ProcessInfo, String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|error| error.to_string())?;

        let creation_time_100ns = match process_creation_time_100ns(handle) {
            Ok(value) => value,
            Err(error) => {
                let _ = CloseHandle(handle);
                return Err(error);
            }
        };

        if !include_image_path {
            let _ = CloseHandle(handle);
            return Ok(ProcessInfo {
                creation_time_100ns,
                exe_path: None,
            });
        }

        let mut buffer = vec![0u16; 2048];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        diagnostics.process_path_queries = diagnostics.process_path_queries.saturating_add(1);
        let _ = CloseHandle(handle);
        result.map_err(|error| error.to_string())?;

        Ok(ProcessInfo {
            creation_time_100ns,
            exe_path: Some(String::from_utf16_lossy(&buffer[..size as usize])),
        })
    }
}

#[derive(Debug, Clone)]
struct ProcessInfo {
    creation_time_100ns: u64,
    exe_path: Option<String>,
}

fn process_creation_time_100ns(handle: windows::Win32::Foundation::HANDLE) -> Result<u64, String> {
    unsafe {
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user)
            .map_err(|error| error.to_string())?;
        Ok(filetime_to_u64(creation))
    }
}

fn filetime_to_u64(value: FILETIME) -> u64 {
    ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64
}

thread_local! {
    static UI_AUTOMATION: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
}

fn is_supported_browser(process_name: &str) -> bool {
    matches!(
        process_name.to_ascii_lowercase().as_str(),
        "chrome.exe"
            | "msedge.exe"
            | "firefox.exe"
            | "brave.exe"
            | "opera.exe"
            | "vivaldi.exe"
            | "chromium.exe"
            | "arc.exe"
            | "floorp.exe"
            | "librewolf.exe"
            | "waterfox.exe"
            | "zen.exe"
    )
}

fn browser_private_marker(process_name: &str) -> String {
    let browser = process_name
        .strip_suffix(".exe")
        .or_else(|| process_name.strip_suffix(".EXE"))
        .unwrap_or(process_name);
    format!("{browser} — private mode")
}

fn has_private_marker(title: &str) -> bool {
    let title = title.to_lowercase();
    [
        "incognito",
        "inprivate",
        "private browsing",
        "private window",
        "navigation privée",
        "navegación privada",
        "приватн",
        "инкогнито",
    ]
    .iter()
    .any(|marker| title.contains(marker))
}

fn titles_indicate_private(native_title: &str, accessible_title: &str) -> bool {
    has_private_marker(native_title)
        || has_private_marker(accessible_title)
        || accessible_title != native_title
}

fn accessible_window_title(hwnd: HWND) -> Result<String, String> {
    UI_AUTOMATION.with(|slot| {
        if slot.borrow().is_none() {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let automation: IUIAutomation =
                    CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                        .map_err(|error| error.to_string())?;
                *slot.borrow_mut() = Some(automation);
            }
        }
        let slot = slot.borrow();
        let automation = slot
            .as_ref()
            .ok_or_else(|| "UI Automation unavailable".to_string())?;
        let name = unsafe {
            automation
                .ElementFromHandle(hwnd)
                .and_then(|element| element.CurrentName())
                .map_err(|error| error.to_string())?
        };
        Ok(name.to_string())
    })
}

fn privacy_safe_window_title(
    hwnd: HWND,
    process_name: &str,
    native_title: Option<String>,
) -> (Option<String>, bool) {
    if !is_supported_browser(process_name) {
        return (native_title, false);
    }
    let marker = || Some(browser_private_marker(process_name));
    let Some(native_title) = native_title else {
        return (marker(), true);
    };
    let accessible_title = match accessible_window_title(hwnd) {
        Ok(title) if !title.trim().is_empty() => title,
        _ => return (marker(), true),
    };
    let accessible = accessible_title.trim();
    let private = titles_indicate_private(&native_title, accessible);
    if private {
        (marker(), true)
    } else {
        (Some(native_title), false)
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

pub fn normalize_exe_path(input: &str) -> String {
    input.trim().replace('/', "\\").to_ascii_lowercase()
}

pub fn process_window_state(
    pid: u32,
    expected_normalized_exe_path: &str,
    snapshot: &mut WindowSnapshot,
    cache: &mut ProcessProbeCache,
    diagnostics: &mut ProcessProbeDiagnostics,
) -> Result<ProcessWindowState, String> {
    let Some(normalized_exe_path) = cache.resolve_normalized_exe_path(pid, diagnostics)? else {
        return Ok(ProcessWindowState::Dead);
    };
    if normalized_exe_path != expected_normalized_exe_path {
        return Ok(ProcessWindowState::Dead);
    }
    if snapshot.has_visible_window(pid) {
        Ok(ProcessWindowState::AliveVisible)
    } else {
        Ok(ProcessWindowState::AliveHidden)
    }
}

impl ProcessProbeCache {
    pub fn resolve_normalized_exe_path(
        &mut self,
        pid: u32,
        diagnostics: &mut ProcessProbeDiagnostics,
    ) -> Result<Option<String>, String> {
        let birth_probe = query_process_info(pid, false, diagnostics)?;
        if let Some(cached) = self.entries.get(&pid) {
            if cached.creation_time_100ns == birth_probe.creation_time_100ns {
                return Ok(Some(cached.normalized_exe_path.clone()));
            }
        }

        let process_info = query_process_info(pid, true, diagnostics)?;
        let Some(exe_path) = process_info.exe_path else {
            self.entries.remove(&pid);
            return Ok(None);
        };
        let normalized_exe_path = normalize_exe_path(&exe_path);
        self.entries.insert(
            pid,
            CachedProcessInfo {
                normalized_exe_path: normalized_exe_path.clone(),
                creation_time_100ns: process_info.creation_time_100ns,
            },
        );
        Ok(Some(normalized_exe_path))
    }

    pub fn retain_pids(&mut self, live_pids: &HashSet<u32>) {
        self.entries.retain(|pid, _| live_pids.contains(pid));
    }
}

/// App identity hashes the canonical exe path (KOS-287): version-looking
/// directories are stripped so an update that moves the exe — Squirrel
/// `app-1.0.x`, per-version browser dirs — keeps one tracked_app row instead
/// of spawning a duplicate. `ark_core::db::canonical_app_key` merges
/// pre-change ids the same way at query time.
pub fn tracked_app_id_for(exe_path: &str) -> String {
    let normalized = normalize_exe_path(exe_path);
    let canonical = ark_core::db::canonical_app_key(&normalized, "");
    let mut hasher = Sha256::new();
    hasher.update(format!("{PLATFORM}:{canonical}").as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_windows_paths() {
        assert_eq!(
            normalize_exe_path("C:/Games/Demo/Game.EXE"),
            "c:\\games\\demo\\game.exe"
        );
    }

    #[test]
    fn tracked_app_id_is_stable() {
        let first = tracked_app_id_for("C:/Games/Demo/Game.EXE");
        let second = tracked_app_id_for("c:\\games\\demo\\game.exe");
        assert_eq!(first, second);
    }

    #[test]
    fn tracked_app_id_survives_version_folder_updates() {
        // KOS-287 regression: a Squirrel-style update moves the exe into a new
        // `app-<version>` dir — the app must keep one id, not double-list.
        let before =
            tracked_app_id_for("C:\\Users\\k\\AppData\\Local\\Discord\\app-1.0.1\\Discord.exe");
        let after =
            tracked_app_id_for("C:\\Users\\k\\AppData\\Local\\Discord\\app-1.0.2\\Discord.exe");
        assert_eq!(before, after);
        let other = tracked_app_id_for("C:\\Other\\Discord.exe");
        assert_ne!(before, other);
    }

    #[test]
    fn chromium_accessibility_prefix_marks_private_without_storing_it() {
        assert!(titles_indicate_private(
            "Facebook - Google Chrome",
            "Incognito - Facebook - Google Chrome"
        ));
        assert!(!titles_indicate_private(
            "Facebook - Google Chrome",
            "Facebook - Google Chrome"
        ));
        assert_eq!(
            browser_private_marker("chrome.exe"),
            "chrome — private mode"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn process_probe_cache_reuses_path_after_birth_time_probe() {
        // Regression: 2026-06-08. Per-session state checks may validate process
        // birth time, but must not query the image path again on every tick.
        let pid = std::process::id();
        let mut cache = ProcessProbeCache::default();
        let mut diagnostics = ProcessProbeDiagnostics::default();

        let first = cache
            .resolve_normalized_exe_path(pid, &mut diagnostics)
            .unwrap();
        assert!(first.is_some());
        assert_eq!(diagnostics.process_path_queries, 1);

        let second = cache
            .resolve_normalized_exe_path(pid, &mut diagnostics)
            .unwrap();
        assert_eq!(second, first);
        assert_eq!(diagnostics.process_path_queries, 1);
    }

    #[test]
    fn window_snapshot_memoizes_visible_lookup_per_pid() {
        let mut snapshot = WindowSnapshot {
            visible_pids: HashSet::from([42]),
            pid_to_has_visible_window: HashMap::new(),
        };

        assert!(snapshot.has_visible_window(42));
        assert!(!snapshot.has_visible_window(7));
        assert_eq!(snapshot.pid_to_has_visible_window.len(), 2);

        assert!(snapshot.has_visible_window(42));
        assert_eq!(snapshot.pid_to_has_visible_window.len(), 2);
    }
}
