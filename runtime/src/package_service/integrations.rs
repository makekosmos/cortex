#[path = "integrations/secret_store.rs"]
pub(crate) mod secret_store;
#[path = "integrations/credential_envelope.rs"]
pub mod credential_envelope;
#[path = "integrations/credential_target.rs"]
mod credential_target;
#[path = "integrations/validation.rs"]
mod validation;
#[path = "integrations/provider.rs"]
mod provider;

use secret_store::{
    clear_package_integration_secret, read_package_integration_secret,
    save_package_integration_secret,
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
        let start_url = login.start_url.clone();
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

    pub async fn complete_integration_login(&self, id: &str, _callback: &str) -> Result<(), PackageError> {
        // No provider-specific code exchanges remain in Engine (the Huawei
        // implementation moved out with the integration packages). The op
        // stays in the contract so hosts get a clean rejection.
        self.integration_package(id)?;
        Err(PackageError::Invalid)
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
        let mut state = self.read_integration_settings();
        self.migrate_vaulted_public_values(package, &integration, &mut state)?;
        let key = Self::bridge_key(&package.id, &package.version);
        let values = state.values.get(&key).cloned().unwrap_or_default();
        let secrets = integration
            .settings
            .iter()
            .filter(|setting| setting.kind.is_secret())
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

    pub async fn set_integration_value(
        &self,
        id: &str,
        setting_key: Option<&str>,
        expected_kind: Option<&str>,
        value: &str,
    ) -> Result<(), PackageError> {
        if !valid_integration_value(value) { return Err(PackageError::Invalid); }
        self.set_integration_value_unlocked(id, setting_key, expected_kind, value).await
    }

    async fn set_integration_value_unlocked(
        &self,
        id: &str,
        setting_key: Option<&str>,
        expected_kind: Option<&str>,
        value: &str,
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
        // The client echoes the kind it rendered; the manifest kind is
        // authoritative — a mismatch means a stale or confused client.
        if expected_kind.is_some_and(|expected| {
            serde_json::to_value(&setting.kind).ok().and_then(|kind| kind.as_str().map(str::to_owned)).as_deref()
                != Some(expected)
        }) {
            return Err(PackageError::Invalid);
        }
        // Storage follows the manifest-declared kind: public values (ник and
        // other plain text) live in integration-settings.json, secrets go to
        // the OS credential vault.
        if setting.kind.is_secret() {
            save_package_integration_secret(
                &package.id,
                &package.version,
                &setting.key,
                value,
            )?;
        } else {
            let mut state = self.read_integration_settings();
            state
                .values
                .entry(Self::bridge_key(&package.id, &package.version))
                .or_default()
                .insert(setting.key.clone(), value.to_owned());
            write_owner_only_json(&self.integration_settings_path(), &state)
                .map_err(|_| PackageError::Persistence)?;
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

    /// A setting that older manifests declared `secret` may still hold a
    /// value in the credential vault after the manifest switched it to a
    /// public kind (`username`/`text`). Move it to plain config once and
    /// delete the vault entry — public values must not sit in the keyring.
    /// Idempotent; called from `integration_provider_snapshots`.
    fn migrate_vaulted_public_values(
        &self,
        package: &InstalledPackage,
        integration: &crate::package_manifest::IntegrationManifest,
        state: &mut PackageIntegrationSettings,
    ) -> Result<(), PackageError> {
        let bridge = Self::bridge_key(&package.id, &package.version);
        let mut dirty = false;
        for setting in &integration.settings {
            if setting.kind.is_secret() {
                continue;
            }
            let Some(value) =
                read_package_integration_secret(&package.id, &package.version, &setting.key)
            else {
                continue;
            };
            if !state
                .values
                .get(&bridge)
                .is_some_and(|values| values.contains_key(&setting.key))
            {
                state
                    .values
                    .entry(bridge.clone())
                    .or_default()
                    .insert(setting.key.clone(), value);
                dirty = true;
            }
            clear_package_integration_secret(&package.id, &package.version, &setting.key)?;
        }
        if dirty {
            write_owner_only_json(&self.integration_settings_path(), state)
                .map_err(|_| PackageError::Persistence)?;
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

    pub(in crate::package_service) fn clear_integration_package_data(
        &self,
        package: &InstalledPackage,
    ) -> Result<(), PackageError> {
        let VersionedManifest::V2(manifest) = &package.manifest else {
            return Ok(());
        };
        let Some(integration) = &manifest.integration else {
            return Ok(());
        };
        let mut state = self.read_integration_settings();
        state.values.remove(&Self::bridge_key(&package.id, &package.version));
        write_owner_only_json(&self.integration_settings_path(), &state)
            .map_err(|_| PackageError::Persistence)?;
        for setting in &integration.settings {
            if setting.kind.is_secret() {
                clear_package_integration_secret(&package.id, &package.version, &setting.key)?;
                // Also drop rotated-secret leftovers written by retired provider
                // login flows (`:huawei-refresh:*` from the removed Huawei login).
                clear_package_integration_secret(&package.id, &package.version, &format!(":huawei-refresh:{}", setting.key))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "integrations/tests.rs"]
mod integration_tests;
