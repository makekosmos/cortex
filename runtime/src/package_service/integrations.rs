#[path = "integrations/cleanup.rs"]
mod cleanup;
#[path = "integrations/secret_store.rs"]
pub(crate) mod secret_store;
#[path = "integrations/credential_envelope.rs"]
pub mod credential_envelope;
#[path = "integrations/credential_target.rs"]
mod credential_target;
#[path = "integrations/validation.rs"]
mod validation;
#[path = "integrations/huawei_login.rs"]
pub(crate) mod huawei_login;

use secret_store::{
    read_package_integration_secret, save_package_integration_secret,
};
use validation::{has_integration_credential, valid_integration_value};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageIntegrationSettings {
    #[serde(default)]
    values: HashMap<String, HashMap<String, String>>,
}

impl PackageService {
    fn integration_settings_path(&self) -> PathBuf {
        self.root.join("integration-settings.json")
    }

    fn read_integration_settings(&self) -> PackageIntegrationSettings {
        let path = self.integration_settings_path();
        let Ok(metadata) = fs::metadata(&path) else {
            return PackageIntegrationSettings::default();
        };
        if !metadata.is_file() || metadata.len() > MAX_DOCUMENT as u64 {
            return PackageIntegrationSettings::default();
        }
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn integration_package(&self, id: &str) -> Result<InstalledPackage, PackageError> {
        self.store
            .list()?
            .into_iter()
            .filter(|package| package.id == id && !package.revoked)
            .filter(|package| {
                matches!(
                    &package.manifest,
                    VersionedManifest::V2(manifest)
                        if manifest.worker_entrypoint().is_some() && manifest.integration.is_some()
                )
            })
            .max_by(|left, right| {
                Version::parse(&left.version)
                    .ok()
                    .cmp(&Version::parse(&right.version).ok())
            })
            .ok_or(PackageError::Invalid)
    }

    pub async fn integration_login_contract(&self, id: &str) -> Result<Value, PackageError> {
        let package = self.integration_package(id)?;
        let VersionedManifest::V2(manifest) = &package.manifest else {
            return Err(PackageError::Invalid);
        };
        let login = manifest
            .integration
            .as_ref()
            .and_then(|integration| integration.login.as_ref())
            .ok_or(PackageError::Invalid)?;
        let start_url = if login.code_exchange.as_deref() == Some("huawei_health") {
            huawei_login::begin(&package.id, &package.version).await?
        } else {
            login.start_url.clone()
        };
        Ok(serde_json::json!({
            "provider": package.id,
            "label": manifest.name,
            "login": {
                "startUrl": start_url,
                "completionUrl": login.completion_url,
                "allowedCookieNames": login.allowed_cookie_names,
                "secretSetting": login.secret_setting,
                "codeExchange": login.code_exchange,
            }
        }))
    }

    pub async fn complete_integration_login(&self, id: &str, callback: &str) -> Result<(), PackageError> {
        let package = self.integration_package(id)?;
        let VersionedManifest::V2(manifest) = &package.manifest else { return Err(PackageError::Invalid); };
        let login = manifest.integration.as_ref().and_then(|integration| integration.login.as_ref())
            .filter(|login| login.code_exchange.as_deref() == Some("huawei_health"))
            .ok_or(PackageError::Invalid)?;
        let mut exchanged = huawei_login::exchange(&package.id, &package.version, callback).await?;
        let _login_lock = huawei_login::lock().await;
        huawei_login::prepare_commit(&package.id, &package.version, &exchanged)?;
        let result = if self.integration_package(id)?.version == package.version {
            self.set_integration_value_unlocked(id, Some(&login.secret_setting), &exchanged.credential).await
        } else { Err(PackageError::Invalid) };
        crate::package_worker_secrets::zeroize_secret(&mut exchanged.credential);
        if result.is_ok() { huawei_login::cancel(&package.id, &package.version)?; }
        result
    }

    fn integration_launch_config(
        &self,
        package: &InstalledPackage,
    ) -> Result<Option<IntegrationLaunchConfig>, PackageError> {
        let VersionedManifest::V2(manifest) = &package.manifest else {
            return Ok(None);
        };
        let Some(integration) = manifest.integration.clone() else {
            return Ok(None);
        };
        let key = Self::bridge_key(&package.id, &package.version);
        let values = self
            .read_integration_settings()
            .values
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let secrets = integration
            .settings
            .iter()
            .filter(|setting| {
                setting.kind == crate::package_manifest::IntegrationSettingKind::Secret
            })
            .filter_map(|setting| {
                read_package_integration_secret(&package.id, &package.version, &setting.key)
                    .map(|secret| (setting.key.clone(), secret))
            })
            .collect();
        Ok(Some(IntegrationLaunchConfig {
            manifest: integration,
            values,
            secrets,
        }))
    }

    pub fn integration_provider_snapshots(&self) -> Result<Vec<Value>, PackageError> {
        let state = self.read_integration_settings();
        let mut providers = Vec::new();
        let mut packages = HashMap::<String, InstalledPackage>::new();
        for package in self.store.list()?.into_iter().filter(|package| !package.revoked) {
            let VersionedManifest::V2(manifest) = &package.manifest else {
                continue;
            };
            if manifest.integration.is_none() {
                continue;
            }
            let replace = packages.get(&package.id).is_none_or(|current| {
                Version::parse(&package.version).ok() > Version::parse(&current.version).ok()
            });
            if replace {
                packages.insert(package.id.clone(), package);
            }
        }
        for package in packages.into_values() {
            let VersionedManifest::V2(manifest) = &package.manifest else {
                continue;
            };
            let Some(integration) = &manifest.integration else {
                continue;
            };
            let key = Self::bridge_key(&package.id, &package.version);
            let values = state.values.get(&key).cloned().unwrap_or_default();
            let first = integration.settings.first();
            let has_credential = has_integration_credential(
                integration,
                &package.id,
                &package.version,
                &values,
            );
            let icon_path = manifest
                .icon
                .as_deref()
                .and_then(|icon| self.store.immutable_asset_path(&package, icon).ok())
                .map(|path| {
                    let path = path.to_string_lossy();
                    path.strip_prefix(r"\\?\").unwrap_or(path.as_ref()).to_owned()
                });
            providers.push(serde_json::json!({
                "id": package.id,
                "label": manifest.name,
                "credentialLabel": first.map(|setting| setting.label.as_str()).unwrap_or("Данные подключения"),
                "credentialUrl": integration.login.as_ref().map(|login| login.start_url.as_str()).unwrap_or(""),
                "hasCredential": has_credential,
                "enabled": package.enabled,
                "iconKey": manifest.id,
                "iconPath": icon_path,
                "packageManaged": true,
                "authMode": if integration.login.is_some() { "browser_login" } else { "credential" },
                "credentialInputType": if first.is_some_and(|setting| setting.kind == crate::package_manifest::IntegrationSettingKind::Text) { "text" } else { "password" },
                "loginCapability": integration.login.as_ref().map(|_| manifest.id.as_str()),
                "settingSchema": integration.settings,
                "settingValues": values,
                "settings": {
                    "intervalMinutes": integration.schedule.as_ref().map(|schedule| schedule.interval_seconds / 60).unwrap_or(0),
                    "syncOnStartup": true,
                    "lastAttemptAt": null,
                    "lastSuccessAt": null,
                    "lastError": null,
                    "importedCount": 0
                }
            }));
        }
        Ok(providers)
    }

    pub async fn set_integration_value(
        &self,
        id: &str,
        setting_key: Option<&str>,
        value: &str,
    ) -> Result<(), PackageError> {
        let _login_lock = huawei_login::lock().await;
        if !valid_integration_value(value) { return Err(PackageError::Invalid); }
        let package = self.integration_package(id)?;
        if let VersionedManifest::V2(manifest) = &package.manifest {
            if let Some(integration) = &manifest.integration {
                if integration.login.as_ref().is_some_and(|login|
                    login.code_exchange.as_deref() == Some("huawei_health")
                        && (setting_key == Some(login.secret_setting.as_str())
                            || (setting_key.is_none() && integration.settings.len() == 1))) {
                    huawei_login::cancel(&package.id, &package.version)?;
                }
            }
        }
        self.set_integration_value_unlocked(id, setting_key, value).await
    }

    async fn set_integration_value_unlocked(
        &self, id: &str, setting_key: Option<&str>, value: &str,
    ) -> Result<(), PackageError> {
        let package = self.integration_package(id)?;
        let VersionedManifest::V2(manifest) = &package.manifest else {
            return Err(PackageError::Invalid);
        };
        let integration = manifest.integration.as_ref().ok_or(PackageError::Invalid)?;
        let setting = match setting_key {
            Some(key) => integration
                .settings
                .iter()
                .find(|setting| setting.key == key),
            None if integration.settings.len() == 1 => integration.settings.first(),
            None => None,
        }
        .ok_or(PackageError::Invalid)?;
        if !valid_integration_value(value) {
            return Err(PackageError::Invalid);
        }
        match setting.kind {
            crate::package_manifest::IntegrationSettingKind::Text => {
                let mut state = self.read_integration_settings();
                state
                    .values
                    .entry(Self::bridge_key(&package.id, &package.version))
                    .or_default()
                    .insert(setting.key.clone(), value.to_owned());
                write_owner_only_json(&self.integration_settings_path(), &state)
                    .map_err(|_| PackageError::Persistence)?;
            }
            crate::package_manifest::IntegrationSettingKind::Secret => {
                save_package_integration_secret(
                    &package.id,
                    &package.version,
                    &setting.key,
                    value,
                )?;
            }
        }
        if package.enabled {
            self.set_enabled(&package.id, &package.version, false).await?;
            self.set_enabled(&package.id, &package.version, true).await?;
        } else {
            let values = self
                .read_integration_settings()
                .values
                .get(&Self::bridge_key(&package.id, &package.version))
                .cloned()
                .unwrap_or_default();
            if has_integration_credential(
                integration,
                &package.id,
                &package.version,
                &values,
            ) {
                self.set_enabled(&package.id, &package.version, true).await?;
            }
        }
        Ok(())
    }

    pub async fn clear_integration_values(&self, id: &str) -> Result<(), PackageError> {
        let _login_lock = huawei_login::lock().await;
        let packages = self
            .store
            .list()?
            .into_iter()
            .filter(|package| {
                package.id == id
                    && !package.revoked
                    && matches!(
                        &package.manifest,
                        VersionedManifest::V2(manifest)
                            if manifest.kind == PackageKind::Source && manifest.integration.is_some()
                    )
            })
            .collect::<Vec<_>>();
        if packages.is_empty() {
            return Err(PackageError::Invalid);
        }
        for package in &packages {
            huawei_login::cancel(&package.id, &package.version)?;
            if package.enabled {
                self.set_enabled(&package.id, &package.version, false).await?;
            }
        }
        if let Some(worker) = self.worker.as_ref() {
            worker.supervisor.revoke_package_secrets(id);
        }
        for package in &packages {
            self.clear_integration_package_data(package)?;
        }
        Ok(())
    }

    pub fn sync_integration_now(&self, id: &str) -> Result<(), PackageError> {
        let package = self.integration_package(id)?;
        self.worker
            .as_ref()
            .ok_or(PackageError::Invalid)?
            .supervisor
            .run_now(&package.id, &package.version)
            .map_err(|_| PackageError::Invalid)
    }
}

#[cfg(test)]
#[path = "integrations/tests.rs"]
mod integration_tests;
