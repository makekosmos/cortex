use super::*;

impl PackageService {
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
}
