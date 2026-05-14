use chrono::Utc;
use std::path::PathBuf;
use std::time::Duration;

use crate::model::{AppUsageTarget, DeviceProfile, UsageSnapshot};

pub trait UsageSampler {
    fn sample(&mut self) -> UsageSnapshot;
}

pub struct WindowsForegroundSampler {
    device: DeviceProfile,
    idle_threshold: Duration,
    #[cfg(target_os = "windows")]
    system: sysinfo::System,
}

impl WindowsForegroundSampler {
    pub fn new(device: DeviceProfile, idle_threshold: Duration) -> Self {
        Self {
            device,
            idle_threshold,
            #[cfg(target_os = "windows")]
            system: sysinfo::System::new_all(),
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn idle_snapshot(&self, idle_ms: u64) -> UsageSnapshot {
        UsageSnapshot::idle(Utc::now(), self.device.clone(), idle_ms)
    }
}

impl UsageSampler for WindowsForegroundSampler {
    fn sample(&mut self) -> UsageSnapshot {
        #[cfg(target_os = "windows")]
        {
            use sysinfo::{Pid, ProcessesToUpdate};

            self.system.refresh_processes(ProcessesToUpdate::All, true);

            let observed_at = Utc::now();
            let idle_ms = unsafe { current_idle_ms() };
            let threshold_ms = self.idle_threshold.as_millis().min(u64::MAX as u128) as u64;

            if idle_ms >= threshold_ms {
                return UsageSnapshot::idle(observed_at, self.device.clone(), idle_ms);
            }

            unsafe {
                let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
                if hwnd.is_null() {
                    return UsageSnapshot::idle(observed_at, self.device.clone(), idle_ms);
                }

                let mut pid = 0u32;
                windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                    hwnd,
                    &mut pid,
                );
                if pid == 0 {
                    return UsageSnapshot::idle(observed_at, self.device.clone(), idle_ms);
                }

                let window_title = read_window_title(hwnd);
                let process = match self.system.process(Pid::from_u32(pid)) {
                    Some(process) => process,
                    None => return UsageSnapshot::idle(observed_at, self.device.clone(), idle_ms),
                };

                let process_name = process.name().to_string_lossy().into_owned();
                let executable_path = process.exe().map(PathBuf::from);
                let app = AppUsageTarget::from_process(
                    executable_path.as_deref(),
                    process_name,
                    window_title,
                    pid,
                );

                UsageSnapshot::foreground(observed_at, self.device.clone(), app)
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let idle_ms = self.idle_threshold.as_millis().min(u64::MAX as u128) as u64;
            self.idle_snapshot(idle_ms)
        }
    }
}

#[cfg(target_os = "windows")]
unsafe fn current_idle_ms() -> u64 {
    use std::mem::size_of;

    let mut info = windows_sys::Win32::UI::Input::KeyboardAndMouse::LASTINPUTINFO {
        cbSize: size_of::<windows_sys::Win32::UI::Input::KeyboardAndMouse::LASTINPUTINFO>()
            as u32,
        dwTime: 0,
    };

    if windows_sys::Win32::UI::Input::KeyboardAndMouse::GetLastInputInfo(&mut info) == 0 {
        return 0;
    }

    let tick_count = windows_sys::Win32::System::SystemInformation::GetTickCount64();
    tick_count.saturating_sub(info.dwTime as u64)
}

#[cfg(target_os = "windows")]
unsafe fn read_window_title(hwnd: windows_sys::Win32::Foundation::HWND) -> Option<String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};

    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return None;
    }

    let mut buffer = vec![0u16; len as usize + 1];
    let written = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
    if written <= 0 {
        return None;
    }

    let title = String::from_utf16_lossy(&buffer[..written as usize]).trim().to_string();
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

#[cfg(not(target_os = "windows"))]
unsafe fn current_idle_ms() -> u64 {
    0
}

#[cfg(not(target_os = "windows"))]
unsafe fn read_window_title(_hwnd: isize) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::WindowsForegroundSampler;
    use crate::model::{DeviceProfile, PLATFORM_WINDOWS};
    use std::time::Duration;

    #[test]
    fn sampler_can_construct() {
        let device = DeviceProfile::new("device", "Device", PLATFORM_WINDOWS);
        let _sampler = WindowsForegroundSampler::new(device, Duration::from_secs(30));
    }
}
