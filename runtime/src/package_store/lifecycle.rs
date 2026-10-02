//! Enabled/revoked lifecycle transitions over `state.json`.

use super::*;

impl PackageStore {
    pub fn enable_worker(&self, id: &str, version: &str) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        let package = state
            .packages
            .iter()
            .find(|p| p.id == id && p.version == version)
            .ok_or_else(|| StoreError::State("package not found".into()))?;
        if package.revoked {
            return Err(StoreError::Revoked);
        }
        if package.manifest.worker_entrypoint().is_none() {
            return Err(StoreError::WorkerRequired);
        }
        for package in &mut state.packages {
            if package.id == id {
                package.enabled = package.version == version;
            }
        }
        self.write_state(&state)
    }

    /// Atomically clear the enabled bit for a set of worker versions.
    /// Callers stop those workers before invoking this method.
    pub fn disable_worker_versions(&self, id: &str, versions: &[String]) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        for version in versions {
            let package = state
                .packages
                .iter()
                .find(|package| package.id == id && package.version == *version)
                .ok_or_else(|| StoreError::State("package not found".into()))?;
            if package.manifest.worker_entrypoint().is_none() {
                return Err(StoreError::WorkerRequired);
            }
        }
        for package in &mut state.packages {
            if package.id == id && versions.iter().any(|version| version == &package.version) {
                package.enabled = false;
            }
        }
        self.write_state(&state)
    }
    pub fn list(&self) -> Result<Vec<InstalledPackage>, StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read_state()?.packages)
    }

    pub fn enable(&self, id: &str, version: &str) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        let package = state
            .packages
            .iter()
            .find(|p| p.id == id && p.version == version)
            .ok_or_else(|| StoreError::State("package not found".into()))?;
        if package.revoked {
            return Err(StoreError::Revoked);
        }
        if !matches!(package.manifest.kind(), PackageKind::App) {
            return Err(StoreError::WorkerRequired);
        }
        for package in &mut state.packages {
            if package.id == id {
                package.enabled = package.version == version;
            }
        }
        self.write_state(&state)
    }
    pub fn disable(&self, id: &str, version: &str) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        let package = state
            .packages
            .iter_mut()
            .find(|p| p.id == id && p.version == version)
            .ok_or_else(|| StoreError::State("package not found".into()))?;
        package.enabled = false;
        self.write_state(&state)
    }
    pub fn uninstall(&self, id: &str, version: &str) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.read_state()?;
        state
            .packages
            .retain(|p| !(p.id == id && p.version == version));
        self.write_state(&state)
    }

    /// Restore an already verified package record without changing its
    /// lifecycle metadata. The immutable blob/unpacked tree is restored by
    /// `install_versioned`; this method only puts the exact prior record back.
    pub fn restore_record(&self, package: &InstalledPackage) -> Result<(), StoreError> {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        package.manifest.validate()?;
        if package.id != package.manifest.id()
            || package.version != package.manifest.version()
            || !is_hash(&package.hash)
            || (package.revoked && package.enabled)
        {
            return Err(StoreError::State("invalid package record".into()));
        }
        let mut state = self.read_state()?;
        state
            .packages
            .retain(|item| item.id != package.id || item.version != package.version);
        state.packages.push(package.clone());
        self.write_state(&state)
    }
    pub fn reconcile_revocations<I, S>(&self, revoked: I) -> Result<(), StoreError>
    where
        I: IntoIterator<Item = (S, S, S)>,
        S: AsRef<str>,
    {
        let _guard = self.mutation.lock().unwrap_or_else(|e| e.into_inner());
        let revoked: Vec<(String, String, String)> = revoked
            .into_iter()
            .map(|(id, version, hash)| {
                (
                    id.as_ref().to_owned(),
                    version.as_ref().to_owned(),
                    hash.as_ref().to_ascii_lowercase(),
                )
            })
            .collect();
        let mut state = self.read_state()?;
        for package in &mut state.packages {
            if revoked.iter().any(|(id, version, hash)| {
                id == &package.id && version == &package.version && hash == &package.hash
            }) {
                package.revoked = true;
                package.enabled = false;
            }
        }
        self.write_state(&state)
    }
}
