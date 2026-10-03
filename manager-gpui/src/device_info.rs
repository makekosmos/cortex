//! Locally-known identity is available without an Engine request. Peer OS and
//! version must come from metadata; never guess them from a computer's name.
use serde_json::Value;

#[path = "../../runtime/src/device_name.rs"]
mod system_name;

#[path = "../../runtime/src/build_metadata.rs"]
mod build_metadata;
pub use build_metadata::version_label;

pub fn product_version_label() -> String {
    // The installed app's bundle version is the product version. Cargo's
    // 0.1.0 is only a crate number and must not be shown as Mundus.
    bundle_short_version().unwrap_or_else(|| build_metadata::compiled().label())
}

fn bundle_short_version() -> Option<String> {
    static CACHED: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| {
            let exe = std::env::current_exe().ok()?;
            let plist = exe.parent()?.parent()?.join("Info.plist");
            let output = std::process::Command::new("/usr/libexec/PlistBuddy")
                .args(["-c", "Print :CFBundleShortVersionString", plist.to_str()?])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let version = String::from_utf8(output.stdout).ok()?.trim().to_string();
            (!version.is_empty()).then_some(version)
        })
        .clone()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
    Linux,
    Android,
    Ios,
    Unknown,
}

impl Platform {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "macos" | "mac" | "darwin" => Self::Mac,
            "windows" | "win32" | "win" => Self::Windows,
            "linux" => Self::Linux,
            "android" => Self::Android,
            "ios" | "iphone" | "ipados" => Self::Ios,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Mac => Some("macOS"),
            Self::Windows => Some("Windows"),
            Self::Linux => Some("Linux"),
            Self::Android => Some("Android"),
            Self::Ios => Some("iOS"),
            Self::Unknown => None,
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Mac => "icons/device-laptop.svg",
            Self::Android => "icons/device-android.svg",
            Self::Ios => "icons/device-iphone.svg",
            Self::Windows | Self::Linux | Self::Unknown => "icons/device-monitor.svg",
        }
    }
}

pub struct LocalDevice {
    pub name: String,
    pub platform: Platform,
}

impl LocalDevice {
    pub fn read() -> Self {
        Self {
            name: system_name::system_device_name(),
            platform: Platform::parse(std::env::consts::OS),
        }
    }

    pub fn caption(&self) -> String {
        format!(
            "{} / {}",
            self.platform.label().unwrap_or("ОС не указана"),
            product_version_label()
        )
    }
}

fn metadata<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
    })
}

pub fn peer_platform(value: &Value) -> Platform {
    Platform::parse(metadata(value, &["platform", "os"]).unwrap_or(""))
}

pub fn peer_caption(value: &Value) -> String {
    let platform = peer_platform(value).label();
    let version = metadata(value, &["manager_version", "app_version", "version"]);
    match (platform, version) {
        (Some(os), Some(version)) => format!("{os} / {version}"),
        (Some(os), None) => os.into(),
        (None, Some(version)) => format!("Версия {version}"),
        (None, None) => "ОС и версия не переданы".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{peer_caption, peer_platform, Platform};
    use serde_json::json;

    #[test]
    fn each_supported_platform_has_a_device_icon() {
        assert_eq!(Platform::parse("Darwin").icon(), "icons/device-laptop.svg");
        assert_eq!(Platform::parse("win32").icon(), "icons/device-monitor.svg");
        assert_eq!(
            Platform::parse("android").icon(),
            "icons/device-android.svg"
        );
        assert_eq!(Platform::parse("ios").icon(), "icons/device-iphone.svg");
    }

    #[test]
    fn peers_display_only_metadata_they_actually_provide() {
        assert_eq!(
            peer_caption(&json!({"platform":"windows", "manager_version":"1.2.3"})),
            "Windows / 1.2.3"
        );
        assert_eq!(peer_caption(&json!({"platform":"macos"})), "macOS");
        let old = json!({"device_name":"DESKTOP-WINDOWS", "device_id":"peer"});
        assert_eq!(peer_platform(&old), Platform::Unknown);
        assert_eq!(peer_caption(&old), "ОС и версия не переданы");
    }
}
