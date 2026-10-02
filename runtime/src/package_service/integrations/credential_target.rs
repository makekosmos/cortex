use super::*;

impl PackageService {
    /// Resolve the active installed manifest target used by credential
    /// replication. Callers never select a package version or setting from an
    /// RPC payload: both are owned by the verified manifest.
    pub(crate) fn integration_credential_target(
        &self,
        id: &str,
    ) -> Result<(String, String), PackageError> {
        let package = self.integration_package(id)?;
        if !package.enabled {
            return Err(PackageError::Invalid);
        }
        let VersionedManifest::V2(manifest) = &package.manifest else {
            return Err(PackageError::Invalid);
        };
        let integration = manifest.integration.as_ref().ok_or(PackageError::Invalid)?;
        let setting = integration
            .login
            .as_ref()
            .map(|login| login.secret_setting.as_str())
            .or_else(|| {
                integration.settings.iter().find(|setting| {
                    setting.required && setting.kind.is_secret()
                }).map(|setting| setting.key.as_str())
            })
            .or_else(|| {
                integration.settings.iter().find(|setting| {
                    setting.kind.is_secret()
                }).map(|setting| setting.key.as_str())
            })
            .ok_or(PackageError::Invalid)?;
        if !integration
            .settings
            .iter()
            .any(|candidate| candidate.key == setting && candidate.kind.is_secret())
        {
            return Err(PackageError::Invalid);
        }
        Ok((package.version, setting.to_owned()))
    }

    /// Persist a decrypted credential while the exact verified package target
    /// is held under the worker mutation lock, then run that provider.
    pub(crate) async fn store_integration_credential_and_sync(
        &self,
        id: &str,
        version: &str,
        setting: &str,
        secret: &str,
    ) -> Result<(), PackageError> {
        let worker = self.worker.as_ref().ok_or(PackageError::Invalid)?;
        let _worker_mutation = worker.mutation.lock().await;
        let package = self.integration_package(id)?;
        if !package.enabled || package.version != version {
            return Err(PackageError::Invalid);
        }
        let VersionedManifest::V2(manifest) = &package.manifest else {
            return Err(PackageError::Invalid);
        };
        let integration = manifest.integration.as_ref().ok_or(PackageError::Invalid)?;
        if !integration
            .settings
            .iter()
            .any(|candidate| candidate.key == setting && candidate.kind.is_secret())
        {
            return Err(PackageError::Invalid);
        }
        worker.supervisor.stop(id, version).await.map_err(|_| PackageError::Persistence)?;
        save_package_integration_secret(id, version, setting, secret)?;
        self.set_worker_enabled_locked(id, version, true, true, Some(&package)).await?;
        worker.supervisor.run_now(id, version).map_err(|_| PackageError::Invalid)
    }
}
