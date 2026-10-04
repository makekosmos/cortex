impl PackageService {
    pub fn enable(&self, id: &str, version: &str) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        self.enable_locked(id, version)
    }

    fn enable_locked(&self, id: &str, version: &str) -> Result<(), PackageError> {
        let mut state = Self::lock(&self.state);
        let entry = Self::current_entry(&mut state, id, version)?;
        let installed = self
            .store
            .list()?
            .into_iter()
            .find(|package| package.id == id && package.version == version)
            .ok_or(PackageError::Invalid)?;
        if entry
            .archive()
            .is_none_or(|archive| !installed.hash.eq_ignore_ascii_case(&archive.sha256))
        {
            return Err(PackageError::Invalid);
        }
        if installed.manifest.worker_entrypoint().is_some() {
            return Err(PackageError::Invalid);
        }
        self.ensure_typed_grant(&installed)?;
        self.store.enable(id, version)?;
        Ok(())
    }

    pub async fn set_enabled(
        &self,
        id: &str,
        version: &str,
        enabled: bool,
    ) -> Result<PackageSummary, PackageError> {
        let installed = self.store.installed(id, version)?;
        if matches!(installed.manifest.kind(), PackageKind::App)
            && installed.manifest.worker_entrypoint().is_none()
        {
            let _mutation = Self::lock(&self.mutation);
            if self.store.installed(id, version)? != installed {
                return Err(PackageError::Persistence);
            }
            if enabled {
                // Dev-installed records (catalog_sequence 0) are absent from
                // the signed catalog by construction; debug builds enable them
                // directly. Release builds keep requiring a catalog entry.
                if cfg!(debug_assertions) && installed.catalog_sequence == 0 {
                    self.ensure_typed_grant(&installed)?;
                    self.store.enable(id, version)?;
                } else {
                    self.enable_locked(id, version)?;
                }
            } else {
                self.disable_locked(id, version)?;
            }
            return Ok(summary(
                self.store.installed(id, version)?,
                self.worker.as_ref(),
                &self.store,
            ));
        }
        let worker = self.worker.as_ref().ok_or(PackageError::Invalid)?;
        let _worker_mutation = worker.mutation.lock().await;
        // Dev-installed records (catalog_sequence 0) are absent from the signed
        // catalog by construction; debug builds still allow enabling them so
        // locally built packages can run end to end. Release builds keep
        // requiring a current catalog entry for every worker launch.
        let require_current_catalog = !cfg!(debug_assertions) || installed.catalog_sequence != 0;
        self.set_worker_enabled_locked(
            id,
            version,
            enabled,
            require_current_catalog,
            Some(&installed),
        )
        .await
    }

    pub fn disable(&self, id: &str, version: &str) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        self.disable_locked(id, version)
    }

    fn disable_locked(&self, id: &str, version: &str) -> Result<(), PackageError> {
        if let Some(worker) = self.worker.as_ref() {
            worker.supervisor.revoke_typed_launch(id, version);
        }
        self.store.disable(id, version)?;
        Ok(())
    }

    fn disable_if_exact(&self, expected: &InstalledPackage) -> Result<bool, PackageError> {
        let _mutation = Self::lock(&self.mutation);
        if self.store.installed(&expected.id, &expected.version)? != *expected {
            return Ok(false);
        }
        self.disable_locked(&expected.id, &expected.version)?;
        Ok(true)
    }

    fn uninstall(&self, id: &str, version: &str) -> Result<(), PackageError> {
        let package = self.store.installed(id, version)?;
        let _mutation = Self::lock(&self.mutation);
        if let Some(worker) = self.worker.as_ref() {
            worker.supervisor.revoke_typed_launch(id, version);
            worker.supervisor.revoke_package_secrets(id);
        }
        self.clear_integration_package_data(&package)?;
        self.store.uninstall(id, version)?;
        Self::lock(&self.typed_grants).remove(&(id.to_owned(), version.to_owned()));
        Ok(())
    }

    pub async fn uninstall_with_worker_stop(
        &self,
        id: &str,
        version: &str,
    ) -> Result<(), PackageError> {
        if let Some(worker) = self.worker.as_ref() {
            let _worker_mutation = worker.mutation.lock().await;
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
        }
        self.uninstall(id, version)
    }
}
