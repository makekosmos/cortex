//! Engine-owned settings shared by Runtime and Manager.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE_NAME: &str = "engine-manager-settings.json";
// MIGRATION(KOS-267): remove after 2026-11-01. Settings file written by the
// Electron shell; it lands in the renamed data dir under its old name.
pub const LEGACY_SHELL_SETTINGS_FILE_NAME: &str = "kepler-shell-settings.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageTrackerSettingSource {
    Engine,
    LegacyMigrated,
    Default,
    TestOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageTrackerStartupConfig {
    pub enabled: bool,
    pub source: UsageTrackerSettingSource,
}

pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_FILE_NAME)
}

fn read_object(path: &Path) -> Option<Map<String, Value>> {
    let value = serde_json::from_str::<Value>(&std::fs::read_to_string(path).ok()?).ok()?;
    value.as_object().cloned()
}

fn write_object_atomic(path: &Path, object: &Map<String, Value>) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))?;
    let temporary = path.with_extension(format!("json.tmp.{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(&Value::Object(object.clone()))?;
    {
        let mut file = std::fs::File::create(&temporary)?;
        use std::io::Write;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let from = temporary
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let to = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        unsafe {
            MoveFileExW(
                PCWSTR(from.as_ptr()),
                PCWSTR(to.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
            .map_err(std::io::Error::from)
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(temporary, path)
    }
}

fn read_engine_usage_enabled(data_dir: &Path) -> Option<bool> {
    read_object(&settings_path(data_dir))?
        .get("usage_tracker")
        .and_then(Value::as_object)
        .and_then(|tracker| tracker.get("enabled"))
        .and_then(Value::as_bool)
}

fn read_legacy_usage_enabled(data_dir: &Path) -> Option<bool> {
    read_object(&data_dir.join(LEGACY_SHELL_SETTINGS_FILE_NAME))?
        .get("usageTrackerEnabled")
        .and_then(Value::as_bool)
}

pub fn resolve_usage_tracker(
    data_dir: &Path,
    test_override: Option<bool>,
) -> UsageTrackerStartupConfig {
    if let Some(enabled) = test_override {
        return UsageTrackerStartupConfig {
            enabled,
            source: UsageTrackerSettingSource::TestOverride,
        };
    }
    if let Some(enabled) = read_engine_usage_enabled(data_dir) {
        return UsageTrackerStartupConfig {
            enabled,
            source: UsageTrackerSettingSource::Engine,
        };
    }
    let Some(enabled) = read_legacy_usage_enabled(data_dir) else {
        return UsageTrackerStartupConfig {
            enabled: true,
            source: UsageTrackerSettingSource::Default,
        };
    };
    let path = settings_path(data_dir);
    let mut object = read_object(&path).unwrap_or_default();
    let tracker = object
        .entry("usage_tracker")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(tracker) = tracker.as_object_mut() {
        tracker.insert("enabled".into(), Value::Bool(enabled));
    } else {
        *tracker = Value::Object(Map::from_iter([("enabled".into(), Value::Bool(enabled))]));
    }
    if let Err(error) = write_object_atomic(&path, &object) {
        eprintln!("[mundus-engine] usage tracker settings migration failed: {error}");
    }
    UsageTrackerStartupConfig {
        enabled,
        source: UsageTrackerSettingSource::LegacyMigrated,
    }
}

pub fn read_settings(data_dir: &Path) -> Value {
    let mut object = read_object(&settings_path(data_dir)).unwrap_or_default();
    let enabled = object
        .get("usage_tracker")
        .and_then(Value::as_object)
        .and_then(|tracker| tracker.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    object
        .entry("usage_tracker")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(tracker) = object
        .get_mut("usage_tracker")
        .and_then(Value::as_object_mut)
    {
        tracker.insert("enabled".into(), Value::Bool(enabled));
    }
    Value::Object(object)
}

pub fn update_settings(
    data_dir: &Path,
    usage_tracker_enabled: Option<bool>,
) -> Result<Value, String> {
    let path = settings_path(data_dir);
    let mut object = read_object(&path).unwrap_or_default();
    if let Some(enabled) = usage_tracker_enabled {
        let tracker = object
            .entry("usage_tracker")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(tracker) = tracker.as_object_mut() {
            tracker.insert("enabled".into(), Value::Bool(enabled));
        } else {
            *tracker = Value::Object(Map::from_iter([("enabled".into(), Value::Bool(enabled))]));
        }
    }
    write_object_atomic(&path, &object).map_err(|error| error.to_string())?;
    Ok(read_settings(data_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_false_migrates_without_mutating_shell_settings() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join(LEGACY_SHELL_SETTINGS_FILE_NAME);
        std::fs::write(
            &legacy,
            r#"{"usageTrackerEnabled":false,"hiddenCommandIds":["focus.start"]}"#,
        )
        .unwrap();
        let before = std::fs::read_to_string(&legacy).unwrap();
        let resolved = resolve_usage_tracker(dir.path(), None);
        assert!(!resolved.enabled);
        assert_eq!(resolved.source, UsageTrackerSettingSource::LegacyMigrated);
        assert_eq!(read_engine_usage_enabled(dir.path()), Some(false));
        assert_eq!(std::fs::read_to_string(legacy).unwrap(), before);
    }

    #[test]
    fn engine_setting_wins_and_updates_preserve_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            settings_path(dir.path()),
            concat!(
                r#"{"future_root":{"x":true,"future":7},"#,
                r#""usage_tracker":{"enabled":true}}"#
            ),
        )
        .unwrap();
        assert!(resolve_usage_tracker(dir.path(), None).enabled);
        let result = update_settings(dir.path(), Some(false)).unwrap();
        assert_eq!(result["future_root"]["future"], 7);
        assert_eq!(result["future_root"]["x"], true);
        assert_eq!(result["usage_tracker"]["enabled"], false);
    }

    #[test]
    fn test_override_is_not_persisted() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!resolve_usage_tracker(dir.path(), Some(false)).enabled);
        assert!(!settings_path(dir.path()).exists());
    }
}
