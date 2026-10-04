use super::{ManifestError, ManifestV2, PackageKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;

mod secret_injection;
pub use secret_injection::{CookieHeaderInjection, IntegrationRequestMethod, SecretInjection};

const MAX_SETTINGS: usize = 64;
const MAX_SETTING_KEY: usize = 64;
const MAX_SETTING_LABEL: usize = 128;
const MAX_SETTING_DESCRIPTION: usize = 512;
const MAX_LOGIN_URL: usize = 2048;
const MAX_COOKIE_NAMES: usize = 64;
const MIN_INTERVAL_SECONDS: u64 = 60;
const MAX_INTERVAL_SECONDS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationManifest {
    #[serde(default)]
    pub settings: Vec<IntegrationSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<BrowserLogin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<IntegrationSchedule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationSetting {
    pub key: String,
    pub label: String,
    #[serde(alias = "type")]
    pub kind: IntegrationSettingKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub injection: Option<SecretInjection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IntegrationSettingKind {
    /// Public login name (ник) — plain integration config, shown unmasked.
    Username,
    /// Generic non-secret value — plain integration config.
    Text,
    /// User-pasted API key — credential vault.
    #[serde(rename = "api_key")]
    ApiKey,
    /// Session or access token — credential vault.
    Token,
    /// Opaque secret that does not fit a more specific kind — credential vault.
    Secret,
}

impl IntegrationSettingKind {
    /// Vault-bound kinds. Everything else is stored as plain integration
    /// config (`integration-settings.json`) and may be rendered unmasked.
    pub fn is_secret(&self) -> bool {
        matches!(self, Self::ApiKey | Self::Token | Self::Secret)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BrowserLogin {
    pub start_url: String,
    pub completion_url: String,
    #[serde(default)]
    pub allowed_cookie_names: Vec<String>,
    pub secret_setting: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_exchange: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationSchedule {
    pub interval_seconds: u64,
}

impl IntegrationManifest {
    pub fn validate(&self, manifest: &ManifestV2) -> Result<(), ManifestError> {
        if !matches!(manifest.kind, PackageKind::Source | PackageKind::App)
            || manifest.declared_worker_entrypoints().is_empty()
        {
            return Err(ManifestError::InvalidField("integration"));
        }
        if self.settings.len() > MAX_SETTINGS {
            return Err(ManifestError::InvalidField("integration.settings"));
        }
        let mut keys = HashSet::new();
        for setting in &self.settings {
            if setting.key.is_empty()
                || setting.key.len() > MAX_SETTING_KEY
                || !setting
                    .key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                || setting.label.is_empty()
                || setting.label.len() > MAX_SETTING_LABEL
                || setting.label.chars().any(char::is_control)
                || setting.description.as_ref().is_some_and(|description| {
                    description.len() > MAX_SETTING_DESCRIPTION
                        || description.chars().any(char::is_control)
                })
                || !keys.insert(&setting.key)
                || setting.injection.as_ref().is_some_and(|injection| {
                    !setting.kind.is_secret()
                        || injection.validate().is_err()
                        || injection.origins().iter().any(|origin| {
                            !manifest.permissions.iter().any(|permission| {
                                permission.capability == "network"
                                    && permission.scopes.iter().any(|scope| scope == origin)
                            })
                        })
                })
            {
                return Err(ManifestError::InvalidField("integration.settings"));
            }
        }
        if let Some(login) = &self.login {
            validate_login(login, &self.settings, manifest)?;
        }
        if self.schedule.as_ref().is_some_and(|schedule| {
            !(MIN_INTERVAL_SECONDS..=MAX_INTERVAL_SECONDS).contains(&schedule.interval_seconds)
        }) {
            return Err(ManifestError::InvalidField("integration.schedule"));
        }
        Ok(())
    }
}

fn validate_login(
    login: &BrowserLogin,
    settings: &[IntegrationSetting],
    manifest: &ManifestV2,
) -> Result<(), ManifestError> {
    if let Some(exchange) = &login.code_exchange {
        let valid = exchange == "huawei_health"
            && login.start_url == "https://oauth-login.cloud.huawei.com/oauth2/v3/authorize"
            && login.completion_url == "hms://redirect_url"
            && login.allowed_cookie_names.is_empty()
            && settings.iter().any(|setting| {
                setting.key == login.secret_setting
                    && setting.kind == IntegrationSettingKind::Secret
                    && matches!(&setting.injection, Some(
                    SecretInjection::Json { origins,
                    body_fields,
                    header_fields,
                    .. },
                )
                    if origins.iter().all(|origin| ["https://sportdata-dre.things.dbankcloud.com/",
                        concat!(
                            "https://sportdata-drru",
                            ".things.dbankcloud.ru/",
                        ), concat!(
                            "https://sportdata-dra.",
                            "things.dbankcloud.com/",
                        ),
                        "https://healthdata.dbankcloud.cn/"].contains(&origin.as_str()))
                    && body_fields.len() == 1 && body_fields.get(
                        "token"
                    ).is_some_and(|field| field == "accessToken")
                    && header_fields.len() == 1 && header_fields.get(
                        "x-huid"
                    ).is_some_and(|field| field == "uid"))
            });
        return valid.then_some(()).ok_or(ManifestError::InvalidField(
            "integration.login.code_exchange",
        ));
    }
    let start_origin = https_origin(&login.start_url)
        .filter(|_| login.start_url.len() <= MAX_LOGIN_URL)
        .ok_or(ManifestError::InvalidField("integration.login.start_url"))?;
    let completion_origin = https_origin(&login.completion_url)
        .filter(|_| login.completion_url.len() <= MAX_LOGIN_URL)
        .ok_or(ManifestError::InvalidField(
            "integration.login.completion_url",
        ))?;
    if login.allowed_cookie_names.is_empty()
        || login.allowed_cookie_names.len() > MAX_COOKIE_NAMES
        || unique_len(&login.allowed_cookie_names) != login.allowed_cookie_names.len()
        || login.allowed_cookie_names.iter().any(|name| {
            name.is_empty()
                || name.len() > 128
                || name.chars().any(char::is_control)
                || name.bytes().any(|byte| byte == b';' || byte == b'=')
        })
    {
        return Err(ManifestError::InvalidField(
            "integration.login.allowed_cookie_names",
        ));
    }
    let Some(setting) = settings
        .iter()
        .find(|setting| setting.key == login.secret_setting)
    else {
        return Err(ManifestError::InvalidField(
            "integration.login.secret_setting",
        ));
    };
    if !setting.kind.is_secret() {
        return Err(ManifestError::InvalidField(
            "integration.login.secret_setting",
        ));
    }
    if !matches!(setting.injection, Some(SecretInjection::Cookies { .. })) {
        return Err(ManifestError::InvalidField(
            "integration.login.secret_setting",
        ));
    }
    if setting
        .injection
        .as_ref()
        .and_then(SecretInjection::cookie_header)
        .is_some_and(|mirror| {
            !login
                .allowed_cookie_names
                .iter()
                .any(|name| name == &mirror.cookie)
        })
    {
        return Err(ManifestError::InvalidField(
            "integration.login.secret_setting",
        ));
    }
    let network_scopes = manifest
        .permissions
        .iter()
        .filter(|permission| permission.capability == "network")
        .flat_map(|permission| permission.scopes.iter());
    for origin in [start_origin, completion_origin] {
        if !network_scopes.clone().any(|scope| {
            network_scope_origin(scope).is_some_and(|scope_origin| scope_origin == origin)
        }) {
            return Err(ManifestError::InvalidField("integration.login.network"));
        }
    }
    Ok(())
}

fn https_origin(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

fn network_scope_origin(value: &str) -> Option<String> {
    let url = Url::parse(value).ok()?;
    if url.path() != "/" || url.query().is_some() {
        return None;
    }
    https_origin(value)
}

fn unique_len<T: Eq + std::hash::Hash>(values: &[T]) -> usize {
    values.iter().collect::<HashSet<_>>().len()
}

#[cfg(test)]
#[path = "integration/tests.rs"]
mod tests;
