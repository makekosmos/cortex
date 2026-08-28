use super::*;

impl PackageWorkerSupervisor {
    #[cfg(windows)]
    pub(super) fn prepare_grant(
        &self,
        key: &(String, String),
        manifest: &PackageManifest,
        hash: String,
        pid: u32,
        generation: u64,
        correlation_id: String,
        roots: &[PathBuf],
        bridge_config: &Option<BridgeWorkerConfig>,
        integration: &Option<IntegrationLaunchConfig>,
    ) -> Result<(Grant, String, BrokerConfig, Vec<u8>), &'static str> {
        let (grant, token) = Grant::derive(
            manifest,
            hash.clone(),
            pid,
            generation,
            correlation_id.clone(),
            roots,
        )
        .map_err(|_| "grant-failed")?;
        let typed_launch_invalid = lock(&self.inner.typed_launches)
            .get(key)
            .is_some_and(|typed| {
                typed.generation > generation || typed.session_id != correlation_id
            });
        if typed_launch_invalid {
            return Err("grant-failed");
        }
        if let Some(typed) = lock(&self.inner.typed_launches).get_mut(key) {
            typed.generation = generation;
        }
        let broker = BrokerConfig::new(
            grant.scopes.get("network").into_iter().flatten(),
            grant
                .scopes
                .get("filesystem.read")
                .into_iter()
                .flatten()
                .chain(grant.scopes.get("filesystem.write").into_iter().flatten())
                .chain(grant.scopes.get("process.spawn").into_iter().flatten())
                .map(PathBuf::from)
                .collect(),
        )
        .map_err(|_| "grant-failed")?;
        let broker = match bridge_config.as_ref() {
            Some(config) => broker
                .with_private_state_root(std::path::Path::new(&config.state_root))
                .map_err(|_| "grant-failed")?,
            None => broker,
        };
        let integration = integration
            .as_ref()
            .map(|config| {
                if config.secrets.keys().any(|key| {
                    !config.manifest.settings.iter().any(|setting| {
                        setting.key == *key && setting.kind == IntegrationSettingKind::Secret
                    })
                }) {
                    return Err("grant-failed");
                }
                let mut handles = HashMap::new();
                for setting in &config.manifest.settings {
                    match setting.kind {
                        IntegrationSettingKind::Text
                            if config.secrets.contains_key(&setting.key) =>
                        {
                            return Err("grant-failed");
                        }
                        IntegrationSettingKind::Secret
                            if config.values.contains_key(&setting.key) =>
                        {
                            return Err("grant-failed");
                        }
                        IntegrationSettingKind::Secret => {
                            if let Some(secret) = config.secrets.get(&setting.key) {
                                let handle = self
                                    .inner
                                    .secrets
                                    .issue(
                                        &manifest.id,
                                        &manifest.version,
                                        generation,
                                        &setting.key,
                                        secret.clone(),
                                    )
                                    .map_err(|_| "grant-failed")?;
                                handles.insert(setting.key.clone(), handle.token());
                            }
                        }
                        IntegrationSettingKind::Text => {}
                    }
                }
                let bootstrap = IntegrationBootstrapConfig {
                    settings: config.manifest.settings.clone(),
                    values: config.values.clone(),
                    secret_handles: handles,
                    schedule: config.manifest.schedule.clone(),
                };
                bootstrap.validate()?;
                Ok(bootstrap)
            })
            .transpose()
            .inspect_err(|_| {
                self.inner
                    .secrets
                    .revoke_generation(&manifest.id, generation);
            })?;
        let bootstrap = BootstrapMessage {
            method: "worker.bootstrap".into(),
            package_id: manifest.id.clone(),
            version: manifest.version.clone(),
            hash,
            pid,
            api_version: self.inner.api_major,
            generation,
            correlation_id,
            token: token.clone(),
            bridge_config: bridge_config.clone(),
            integration,
        };
        let mut line = serde_json::to_vec(&bootstrap).map_err(|_| {
            self.inner
                .secrets
                .revoke_generation(&manifest.id, generation);
            "bootstrap-serialize-failed"
        })?;
        line.push(b'\n');
        Ok((grant, token, broker, line))
    }
}
