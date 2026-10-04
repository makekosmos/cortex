impl PackageService {
    fn bridge_config_path(&self) -> PathBuf {
        self.root.join("bridge-config.json")
    }

    fn bridge_key(id: &str, version: &str) -> String {
        format!("{id}@{version}")
    }

    fn bridge_roots(&self, id: &str, version: &str) -> Result<Vec<PathBuf>, PackageError> {
        let config = Self::lock(&self.bridge_configs)
            .configs
            .get(&Self::bridge_key(id, version))
            .cloned()
            .ok_or(PackageError::Invalid)?;
        Ok(vec![
            PathBuf::from(config.vault_root),
            self.root.join("bridge-state").join(id).join(version),
        ])
    }

    pub fn bridge_config(
        &self,
        id: &str,
        version: &str,
    ) -> Result<BridgeConfigStatus, PackageError> {
        let package = self.store.installed(id, version)?;
        if !matches!(package.manifest.kind(), PackageKind::Bridge) {
            return Err(PackageError::Invalid);
        }
        let config = Self::lock(&self.bridge_configs)
            .configs
            .get(&Self::bridge_key(id, version))
            .cloned();
        Ok(BridgeConfigStatus {
            configured: config.is_some(),
            config,
            worker_state: self
                .worker
                .as_ref()
                .map(|worker| worker.supervisor.health(id, version).state)
                .unwrap_or(WorkerState::Stopped),
            worker_status: self.worker.as_ref().and_then(|worker| {
                worker
                    .supervisor
                    .diagnostics()
                    .into_iter()
                    .find(|item| item.id == id && item.version == version)
                    .and_then(|item| item.bridge_status)
            }),
        })
    }

    /// Stop before changing the exact filesystem root, then persist the new grant source.
    pub async fn set_bridge_config(
        &self,
        id: &str,
        version: &str,
        config: BridgeConfig,
    ) -> Result<BridgeConfigStatus, PackageError> {
        validate_bridge_config(&config)?;
        let package = self.store.installed(id, version)?;
        if !matches!(package.manifest.kind(), PackageKind::Bridge) {
            return Err(PackageError::Invalid);
        }
        let was_enabled = package.enabled;
        if let Some(worker) = self.worker.as_ref() {
            let _worker_mutation = worker.mutation.lock().await;
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
        }
        {
            let mut configs = Self::lock(&self.bridge_configs);
            ensure_owner_only_directory(&self.root.join("bridge-state").join(id).join(version))
                .map_err(|_| PackageError::Persistence)?;
            configs
                .configs
                .insert(Self::bridge_key(id, version), config);
            write_owner_only_json(&self.bridge_config_path(), &*configs)
                .map_err(|_| PackageError::Persistence)?;
        }
        if was_enabled {
            // The old grant was revoked by stop(); this starts with only the newly persisted roots.
            self.set_enabled(id, version, true).await?;
        }
        self.bridge_config(id, version)
    }
}
