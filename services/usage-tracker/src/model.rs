use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const PLATFORM_WINDOWS: &str = "windows";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceProfile {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
}

impl DeviceProfile {
    pub fn new(
        device_id: impl Into<String>,
        device_name: impl Into<String>,
        platform: impl Into<String>,
    ) -> Self {
        Self {
            device_id: device_id.into(),
            device_name: device_name.into(),
            platform: platform.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppUsageTarget {
    pub app_id: String,
    pub display_name: String,
    pub process_name: String,
    pub executable_path: Option<PathBuf>,
    pub normalized_exe_path: Option<String>,
    pub window_title: Option<String>,
    pub pid: u32,
}

impl AppUsageTarget {
    pub fn from_process(
        executable_path: Option<&Path>,
        process_name: impl Into<String>,
        window_title: Option<String>,
        pid: u32,
    ) -> Self {
        let process_name = process_name.into();
        let normalized_exe_path = executable_path.map(normalize_exe_path);
        let app_id = build_app_id(executable_path, &process_name);
        let display_name = display_name_from_process(executable_path, &process_name);

        Self {
            app_id,
            display_name,
            process_name,
            executable_path: executable_path.map(Path::to_path_buf),
            normalized_exe_path,
            window_title,
            pid,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdleUsage {
    pub idle_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsageTarget {
    Foreground(AppUsageTarget),
    Idle(IdleUsage),
}

impl UsageTarget {
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageKind {
    Foreground,
    Idle,
}

impl UsageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            UsageKind::Foreground => "foreground",
            UsageKind::Idle => "idle",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageSnapshot {
    pub observed_at: DateTime<Utc>,
    pub device: DeviceProfile,
    pub target: UsageTarget,
}

impl UsageSnapshot {
    pub fn foreground(
        observed_at: DateTime<Utc>,
        device: DeviceProfile,
        app: AppUsageTarget,
    ) -> Self {
        Self {
            observed_at,
            device,
            target: UsageTarget::Foreground(app),
        }
    }

    pub fn idle(observed_at: DateTime<Utc>, device: DeviceProfile, idle_ms: u64) -> Self {
        Self {
            observed_at,
            device,
            target: UsageTarget::Idle(IdleUsage { idle_ms }),
        }
    }
}

pub fn normalize_exe_path(executable_path: &Path) -> String {
    executable_path
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase()
}

pub fn build_app_id(executable_path: Option<&Path>, process_name: &str) -> String {
    match executable_path {
        Some(path) => format!("exe:{}", normalize_exe_path(path)),
        None => format!("process:{}", process_name.trim().to_lowercase()),
    }
}

pub fn display_name_from_process(executable_path: Option<&Path>, process_name: &str) -> String {
    if let Some(path) = executable_path {
        if let Some(file_stem) = path.file_stem() {
            let stem = file_stem.to_string_lossy().trim().to_string();
            if !stem.is_empty() {
                return stem;
            }
        }
    }

    process_name.trim().to_string()
}

pub fn duration_to_ms(duration: Duration) -> i64 {
    duration.as_millis().min(i64::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::{build_app_id, display_name_from_process, normalize_exe_path};
    use std::path::Path;

    #[test]
    fn builds_exe_based_identity() {
        let path = Path::new(r"C:\Games\Example\App.exe");
        assert_eq!(build_app_id(Some(path), "App"), "exe:c:\\games\\example\\app.exe");
        assert_eq!(display_name_from_process(Some(path), "App"), "App");
        assert_eq!(normalize_exe_path(path), "c:\\games\\example\\app.exe");
    }

    #[test]
    fn falls_back_to_process_name() {
        assert_eq!(build_app_id(None, "Explorer"), "process:explorer");
        assert_eq!(display_name_from_process(None, "Explorer"), "Explorer");
    }
}
