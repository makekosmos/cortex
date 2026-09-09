use super::secret_store::clear_package_integration_secret;
use super::*;

impl PackageService {
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
            if setting.kind == crate::package_manifest::IntegrationSettingKind::Secret {
                clear_package_integration_secret(&package.id, &package.version, &setting.key)?;
                clear_package_integration_secret(&package.id, &package.version, &format!(":huawei-refresh:{}", setting.key))?;
            }
        }
        huawei_login::cancel(&package.id, &package.version)?;
        Ok(())
    }
}
