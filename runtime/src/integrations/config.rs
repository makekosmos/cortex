use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub(crate) const CONFIG_FILE: &str = "integrations.json";
pub(crate) const KEYRING_SERVICE: &str = "kosmos-kepler";
pub(crate) const WORKOUT_TYPE_ID: &str = "workout_obj";
pub(crate) const TIME_ENTRY_TYPE_ID: &str = "time_entry_obj";
pub(crate) const CODING_SUBMISSION_TYPE_ID: &str = "coding_submission_obj";
pub(crate) const CODING_PROFILE_TYPE_ID: &str = "coding_profile_obj";
pub(crate) const HEVY_BASE_URL: &str = "https://api.hevyapp.com";
pub(crate) const TOGGL_BASE_URL: &str = "https://api.track.toggl.com/api/v9";
pub(crate) const LEETCODE_GRAPHQL_URL: &str = "https://leetcode.com/graphql";
pub(crate) const CODEWARS_BASE_URL: &str = "https://www.codewars.com/api/v1";
pub(crate) const ALLOWED_INTERVALS: &[u64] = &[0, 15, 60, 360, 1440];

pub(crate) static CONFIG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
pub(crate) static SYNC_LOCKS: OnceLock<[tokio::sync::Mutex<()>; 4]> = OnceLock::new();

pub(crate) fn config_lock() -> &'static Mutex<()> {
    CONFIG_LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn sync_lock(index: usize) -> &'static tokio::sync::Mutex<()> {
    &SYNC_LOCKS.get_or_init(|| std::array::from_fn(|_| tokio::sync::Mutex::new(())))[index]
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Hevy,
    Toggl,
    Leetcode,
    Codewars,
}

impl Provider {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "hevy" => Ok(Self::Hevy),
            "toggl" => Ok(Self::Toggl),
            "leetcode" => Ok(Self::Leetcode),
            "codewars" => Ok(Self::Codewars),
            _ => Err(format!(
                "РќРµРёР·РІРµСЃС‚РЅР°СЏ РёРЅС‚РµРіСЂР°С†РёСЏ: {value}"
            )),
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Hevy => "hevy",
            Self::Toggl => "toggl",
            Self::Leetcode => "leetcode",
            Self::Codewars => "codewars",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Hevy => "Hevy",
            Self::Toggl => "Toggl Track",
            Self::Leetcode => "LeetCode",
            Self::Codewars => "Codewars",
        }
    }

    pub(crate) fn credential_label(self) -> &'static str {
        match self {
            Self::Hevy => "API-РєР»СЋС‡",
            Self::Toggl => "API-С‚РѕРєРµРЅ",
            Self::Leetcode => "РЎРµСЃСЃРёСЏ LeetCode",
            Self::Codewars => "РРјСЏ РїРѕР»СЊР·РѕРІР°С‚РµР»СЏ",
        }
    }

    pub(crate) fn credential_url(self) -> &'static str {
        match self {
            Self::Hevy => "https://hevy.com/settings?developer",
            Self::Toggl => "https://track.toggl.com/profile",
            Self::Leetcode => "https://leetcode.com/accounts/login/",
            Self::Codewars => "https://www.codewars.com/users/",
        }
    }

    pub(crate) fn keyring_user(self) -> &'static str {
        match self {
            Self::Hevy => "integration-hevy-api-key",
            Self::Toggl => "integration-toggl-api-token",
            Self::Leetcode => "integration-leetcode-session",
            Self::Codewars => "integration-codewars-username",
        }
    }

    pub(crate) fn sync_lock_index(self) -> usize {
        match self {
            Self::Hevy => 0,
            Self::Toggl => 1,
            Self::Leetcode => 2,
            Self::Codewars => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderSettings {
    pub interval_minutes: u64,
    pub sync_on_startup: bool,
    pub last_attempt_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub imported_count: u64,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            interval_minutes: 0,
            sync_on_startup: false,
            last_attempt_at: None,
            last_success_at: None,
            last_error: None,
            imported_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IntegrationsConfig {
    pub hevy: ProviderSettings,
    pub toggl: ProviderSettings,
    pub leetcode: ProviderSettings,
    pub codewars: ProviderSettings,
    pub body_weight_kg: Option<f64>,
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self {
            hevy: ProviderSettings {
                interval_minutes: 360,
                sync_on_startup: true,
                ..ProviderSettings::default()
            },
            toggl: ProviderSettings {
                interval_minutes: 60,
                sync_on_startup: false,
                ..ProviderSettings::default()
            },
            leetcode: ProviderSettings {
                interval_minutes: 1440,
                sync_on_startup: true,
                ..ProviderSettings::default()
            },
            codewars: ProviderSettings {
                interval_minutes: 1440,
                sync_on_startup: true,
                ..ProviderSettings::default()
            },
            body_weight_kg: None,
        }
    }
}

impl IntegrationsConfig {
    pub(crate) fn provider(&self, provider: Provider) -> &ProviderSettings {
        match provider {
            Provider::Hevy => &self.hevy,
            Provider::Toggl => &self.toggl,
            Provider::Leetcode => &self.leetcode,
            Provider::Codewars => &self.codewars,
        }
    }

    pub(crate) fn provider_mut(&mut self, provider: Provider) -> &mut ProviderSettings {
        match provider {
            Provider::Hevy => &mut self.hevy,
            Provider::Toggl => &mut self.toggl,
            Provider::Leetcode => &mut self.leetcode,
            Provider::Codewars => &mut self.codewars,
        }
    }
}

pub(crate) fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CONFIG_FILE)
}

pub(crate) fn read_config_unlocked(data_dir: &Path) -> IntegrationsConfig {
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

pub(crate) fn write_config_unlocked(
    data_dir: &Path,
    config: &IntegrationsConfig,
) -> Result<(), String> {
    fs::create_dir_all(data_dir)
        .map_err(|error| format!("РќРµ СѓРґР°Р»РѕСЃСЊ СЃРѕР·РґР°С‚СЊ РїР°РїРєСѓ: {error}"))?;
    let target = config_path(data_dir);
    let temporary = target.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(config).map_err(|error| {
        format!("РќРµ СѓРґР°Р»РѕСЃСЊ СЃРµСЂРёР°Р»РёР·РѕРІР°С‚СЊ РЅР°СЃС‚СЂРѕР№РєРё: {error}")
    })?;
    fs::write(&temporary, json).map_err(|error| {
        format!("РќРµ СѓРґР°Р»РѕСЃСЊ СЃРѕС…СЂР°РЅРёС‚СЊ РЅР°СЃС‚СЂРѕР№РєРё: {error}")
    })?;
    fs::rename(&temporary, &target).map_err(|error| {
        format!("РќРµ СѓРґР°Р»РѕСЃСЊ РїСЂРёРјРµРЅРёС‚СЊ РЅР°СЃС‚СЂРѕР№РєРё: {error}")
    })
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
