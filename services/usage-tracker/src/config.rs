use std::env;
use std::path::PathBuf;
use std::time::Duration;

use crate::model::{DeviceProfile, PLATFORM_WINDOWS};

const DEFAULT_POLL_INTERVAL_SECS: u64 = 10;
const DEFAULT_IDLE_THRESHOLD_SECS: u64 = 120;
const DEFAULT_APP_NAME: &str = "usage-tracker";
const DEFAULT_DATA_DIR_NAME: &str = "usage-tracker";

pub const ENV_DB_PATH: &str = "USAGE_TRACKER_DB_PATH";
pub const ENV_DATA_DIR: &str = "USAGE_TRACKER_DATA_DIR";
pub const ENV_LOCK_PATH: &str = "USAGE_TRACKER_LOCK_PATH";
pub const ENV_DEVICE_ID: &str = "USAGE_TRACKER_DEVICE_ID";
pub const ENV_DEVICE_NAME: &str = "USAGE_TRACKER_DEVICE_NAME";
pub const ENV_APP_NAME: &str = "USAGE_TRACKER_APP_NAME";
pub const ENV_POLL_INTERVAL_SECS: &str = "USAGE_TRACKER_POLL_INTERVAL_SECONDS";
pub const ENV_IDLE_THRESHOLD_SECS: &str = "USAGE_TRACKER_IDLE_THRESHOLD_SECONDS";

#[derive(Clone, Debug)]
pub struct Config {
    pub db_path: PathBuf,
    pub data_dir: PathBuf,
    pub lock_path: PathBuf,
    pub device_id: String,
    pub device_name: String,
    pub app_name: String,
    pub poll_interval: Duration,
    pub idle_threshold: Duration,
}

impl Config {
    pub fn from_env_and_args() -> Result<Self, String> {
        let args: Vec<String> = env::args().skip(1).collect();

        let db_path = read_path(&args, "--db-path", ENV_DB_PATH)
            .ok_or_else(|| format!("{ENV_DB_PATH} or --db-path is required"))?;
        let data_dir = read_path(&args, "--data-dir", ENV_DATA_DIR).unwrap_or_else(default_data_dir);
        let lock_path = read_path(&args, "--lock-path", ENV_LOCK_PATH)
            .unwrap_or_else(|| data_dir.join("usage-tracker.lock.db"));
        let device_id = read_string(&args, "--device-id", ENV_DEVICE_ID)
            .unwrap_or_else(default_device_id);
        let device_name = read_string(&args, "--device-name", ENV_DEVICE_NAME)
            .unwrap_or_else(default_device_name);
        let app_name = read_string(&args, "--app-name", ENV_APP_NAME)
            .unwrap_or_else(|| DEFAULT_APP_NAME.to_string());
        let poll_interval = read_duration_secs(
            &args,
            "--poll-interval-seconds",
            ENV_POLL_INTERVAL_SECS,
            DEFAULT_POLL_INTERVAL_SECS,
        )?;
        let idle_threshold = read_duration_secs(
            &args,
            "--idle-threshold-seconds",
            ENV_IDLE_THRESHOLD_SECS,
            DEFAULT_IDLE_THRESHOLD_SECS,
        )?;

        Ok(Self {
            db_path,
            data_dir,
            lock_path,
            device_id,
            device_name,
            app_name,
            poll_interval,
            idle_threshold,
        })
    }

    pub fn device_profile(&self) -> DeviceProfile {
        DeviceProfile::new(self.device_id.clone(), self.device_name.clone(), PLATFORM_WINDOWS)
    }
}

fn read_string(args: &[String], flag: &str, env_name: &str) -> Option<String> {
    if let Some(value) = arg_value(args, flag) {
        return Some(value);
    }

    env::var(env_name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_path(args: &[String], flag: &str, env_name: &str) -> Option<PathBuf> {
    read_string(args, flag, env_name).map(PathBuf::from)
}

fn read_duration_secs(
    args: &[String],
    flag: &str,
    env_name: &str,
    default: u64,
) -> Result<Duration, String> {
    let raw = read_string(args, flag, env_name).unwrap_or_else(|| default.to_string());
    let secs: u64 = raw
        .parse()
        .map_err(|_| format!("{flag} / {env_name} must be a positive integer"))?;

    Ok(Duration::from_secs(secs))
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    let with_equals = format!("{flag}=");

    for (index, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix(&with_equals) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        if arg == flag {
            if let Some(value) = args.get(index + 1) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    None
}

fn default_data_dir() -> PathBuf {
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("Kepler")
            .join(DEFAULT_DATA_DIR_NAME);
    }

    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(DEFAULT_DATA_DIR_NAME)
}

fn default_device_name() -> String {
    env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-device".to_string())
}

fn default_device_id() -> String {
    default_device_name().to_lowercase().replace([' ', '\t'], "-")
}

#[cfg(test)]
mod tests {
    use super::{arg_value, read_duration_secs};

    #[test]
    fn arg_value_supports_equals_and_split_forms() {
        let args = vec![
            "--db-path=C:\\ark.db".to_string(),
            "--poll-interval-seconds".to_string(),
            "15".to_string(),
        ];

        assert_eq!(arg_value(&args, "--db-path"), Some("C:\\ark.db".to_string()));
        assert_eq!(
            arg_value(&args, "--poll-interval-seconds"),
            Some("15".to_string())
        );
    }

    #[test]
    fn read_duration_secs_parses_values() {
        let args = vec!["--poll-interval-seconds".to_string(), "12".to_string()];
        let duration = read_duration_secs(&args, "--poll-interval-seconds", "ENV", 10)
            .expect("parse duration");
        assert_eq!(duration.as_secs(), 12);
    }
}
