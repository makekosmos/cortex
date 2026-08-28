use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const CONFIG_FILE: &str = "integrations.json";
static CONFIG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct IntegrationsConfig {
    pub(crate) body_weight_kg: Option<f64>,
}

fn config_lock() -> &'static Mutex<()> {
    CONFIG_LOCK.get_or_init(|| Mutex::new(()))
}

fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CONFIG_FILE)
}

fn read_config_unlocked(data_dir: &Path) -> IntegrationsConfig {
    fs::read_to_string(config_path(data_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub(crate) fn read_config(data_dir: &Path) -> IntegrationsConfig {
    let _guard = config_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    read_config_unlocked(data_dir)
}

fn write_config_unlocked(data_dir: &Path, config: &IntegrationsConfig) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|error| format!("Не удалось создать папку: {error}"))?;
    let target = config_path(data_dir);
    let temporary = target.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(config)
        .map_err(|error| format!("Не удалось сериализовать настройки: {error}"))?;
    fs::write(&temporary, json)
        .map_err(|error| format!("Не удалось сохранить настройки: {error}"))?;
    fs::rename(&temporary, &target)
        .map_err(|error| format!("Не удалось применить настройки: {error}"))
}

pub(crate) fn mutate_config(
    data_dir: &Path,
    mutate: impl FnOnce(&mut IntegrationsConfig) -> Result<(), String>,
) -> Result<IntegrationsConfig, String> {
    let _guard = config_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut config = read_config_unlocked(data_dir);
    mutate(&mut config)?;
    write_config_unlocked(data_dir, &config)?;
    Ok(config)
}
