//! Engine-owned, per-device appearance preferences. Never stored in ARK/sync.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    future::Future,
    io::Write,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

const FILE_NAME: &str = "appearance-settings.json";
const WALLPAPER_TTL: Duration = Duration::from_secs(30);
const WALLPAPER_PENDING: &str = "desktop wallpaper accent is pending";
// Keys from the pinned Imago ThemeDef table (crates/imago-gpui/src/palettes.rs).
// Keep this list in step with the pinned Imago revision when themes change.
const THEME_IDS: &[&str] = &[
    "default",
    "t3-chat",
    "claymorphism",
    "claude",
    "graphite",
    "amethyst-haze",
    "vercel",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppearanceSettings {
    pub schema_version: u8,
    pub mode: String,
    pub light_theme: String,
    pub dark_theme: String,
    pub accent_source: String,
    pub accent_color: Option<String>,
    pub follow_apps: bool,
    pub material: String,
    pub font_family: String,
    pub font_size: f32,
    pub revision: u64,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            mode: "dark".into(),
            light_theme: "default".into(),
            dark_theme: "default".into(),
            accent_source: "theme".into(),
            accent_color: None,
            follow_apps: false,
            material: "default".into(),
            font_family: "Inter".into(),
            font_size: 13.0,
            revision: 0,
        }
    }
}

pub fn is_appearance_operation(operation: &str) -> bool {
    matches!(operation, "appearance.get" | "appearance.set")
}

fn materials() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        match windows_build() {
            Some(22000..) => &["default", "opaque", "acrylic", "mica"],
            Some(17763..) => &["default", "opaque", "acrylic"],
            _ => &["default", "opaque"],
        }
    }
    #[cfg(target_os = "macos")]
    {
        &["default", "frosted", "opaque"]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        &["default", "opaque"]
    }
}

#[cfg(target_os = "windows")]
fn windows_build() -> Option<u32> {
    // RtlGetVersion reports the actual OS build, unlike manifest-sensitive
    // GetVersionEx. Mica starts with Windows 11 (build 22000); older or
    // unknown builds must not advertise it.
    #[repr(C)]
    struct VersionInfo {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service_pack: [u16; 128],
    }
    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(info: *mut VersionInfo) -> i32;
    }
    let mut info = VersionInfo {
        size: std::mem::size_of::<VersionInfo>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        service_pack: [0; 128],
    };
    let status = unsafe { RtlGetVersion(&mut info) };
    (status >= 0 && info.major == 10).then_some(info.build)
}

fn valid_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

fn validate(settings: &AppearanceSettings) -> Result<(), String> {
    if settings.schema_version != 1 {
        return Err("schema_version must be 1".into());
    }
    if !["system", "light", "dark"].contains(&settings.mode.as_str()) {
        return Err("invalid mode".into());
    }
    if !THEME_IDS.contains(&settings.light_theme.as_str()) {
        return Err("invalid light_theme".into());
    }
    if !THEME_IDS.contains(&settings.dark_theme.as_str()) {
        return Err("invalid dark_theme".into());
    }
    if !["theme", "custom", "wallpaper"].contains(&settings.accent_source.as_str()) {
        return Err("invalid accent_source".into());
    }
    if settings
        .accent_color
        .as_deref()
        .is_some_and(|color| !valid_color(color))
    {
        return Err("accent_color must be #RRGGBB or null".into());
    }
    if settings.accent_source == "custom" && settings.accent_color.is_none() {
        return Err("custom accent requires accent_color".into());
    }
    if settings.accent_source == "wallpaper" && !cfg!(target_os = "macos") {
        return Err("desktop wallpaper accent is unsupported on this platform".into());
    }
    if !materials().contains(&settings.material.as_str()) {
        return Err("material is unsupported on this platform".into());
    }
    if settings.font_family.is_empty()
        || settings.font_family.len() > 128
        || settings.font_family.chars().any(char::is_control)
    {
        return Err("invalid font_family".into());
    }
    if !settings.font_size.is_finite() || !(11.0..=18.0).contains(&settings.font_size) {
        return Err("font_size must be 11..18".into());
    }
    Ok(())
}

fn read(path: &Path) -> Result<AppearanceSettings, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppearanceSettings::default())
        }
        Err(error) => return Err(error.to_string()),
    };
    let settings: AppearanceSettings = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid appearance settings file: {error}"))?;
    validate(&settings)?;
    Ok(settings)
}

fn persist(path: &Path, settings: &AppearanceSettings) -> Result<(), String> {
    let parent = path.parent().ok_or("appearance path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(".{FILE_NAME}.{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::{
                core::PCWSTR,
                Win32::Storage::FileSystem::{
                    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
                },
            };
            let from: Vec<_> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe {
                MoveFileExW(
                    PCWSTR(from.as_ptr()),
                    PCWSTR(to.as_ptr()),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
                .map_err(|error| error.to_string())?;
            }
        }
        #[cfg(not(windows))]
        fs::rename(&temporary, path).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        fs::File::open(parent)
            .and_then(|dir| dir.sync_all())
            .map_err(|error| error.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

pub struct AppearanceStore {
    path: PathBuf,
    write_lock: Mutex<()>,
    wallpaper: Arc<WallpaperCache>,
}

include!("appearance/wallpaper.rs");

impl AppearanceStore {
    pub fn new(data_dir: PathBuf) -> Self {
        let sampler: WallpaperSampler = Arc::new(|| Box::pin(desktop_wallpaper_accent()));
        Self::with_sampler(data_dir, sampler, WALLPAPER_TTL)
    }

    fn with_sampler(data_dir: PathBuf, sampler: WallpaperSampler, ttl: Duration) -> Self {
        Self {
            path: data_dir.join(FILE_NAME),
            write_lock: Mutex::new(()),
            wallpaper: Arc::new(WallpaperCache {
                state: Mutex::new(WallpaperCacheState::default()),
                sampler,
                ttl,
            }),
        }
    }

    pub async fn get(&self, params: &Value) -> Result<Value, String> {
        if !params.is_null() && params.as_object().is_none_or(|object| !object.is_empty()) {
            return Err("appearance.get params must be empty".into());
        }
        let settings = read(&self.path)?;
        Ok(self.response(settings).await)
    }

    pub async fn set(&self, params: &Value) -> Result<Value, String> {
        let patch = params
            .as_object()
            .ok_or("appearance.set params must be a patch object")?;
        if patch.is_empty() {
            return Err("appearance.set patch must not be empty".into());
        }
        if patch.contains_key("revision") || patch.contains_key("schema_version") {
            return Err("revision and schema_version are server owned".into());
        }
        let _guard = self.write_lock.lock().await;
        let mut next =
            serde_json::to_value(read(&self.path)?).map_err(|error| error.to_string())?;
        let object = next.as_object_mut().expect("serialized settings object");
        for (key, value) in patch {
            if !object.contains_key(key) {
                return Err(format!("unknown appearance field: {key}"));
            }
            object.insert(key.clone(), value.clone());
        }
        let mut settings: AppearanceSettings = serde_json::from_value(next)
            .map_err(|error| format!("invalid appearance patch: {error}"))?;
        validate(&settings)?;
        settings.revision = settings
            .revision
            .checked_add(1)
            .ok_or("appearance revision exhausted")?;
        persist(&self.path, &settings)?;
        drop(_guard);
        Ok(self.response(settings).await)
    }

    async fn response(&self, settings: AppearanceSettings) -> Value {
        let wants_wallpaper = settings.accent_source == "wallpaper";
        let mut value = json!({
            "settings": settings,
            "capabilities": {
                "materials": materials(),
                "wallpaper_accent": cfg!(target_os = "macos"),
            },
            "wallpaper_accent": null,
        });
        if wants_wallpaper {
            match self.wallpaper.snapshot().await {
                Ok(color) => value["wallpaper_accent"] = json!(color),
                Err(error) => value["wallpaper_error"] = json!(error),
            }
        }
        value
    }
}

#[cfg(test)]
mod tests {
    include!("appearance/tests.rs");
}
