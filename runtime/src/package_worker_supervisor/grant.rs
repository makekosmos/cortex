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
                typed.generation != generation || typed.session_id != correlation_id
            });
        if typed_launch_invalid {
            return Err("grant-failed");
        }
        let broker = BrokerConfig::new(
            grant.scopes.get("network").into_iter().flatten(),
            grant
                .scopes
                .get("filesystem.read")
                .into_iter()
                .flatten()
                .chain(grant.scopes.get("filesystem.write").into_iter().flatten())
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
        };
        let mut line = serde_json::to_vec(&bootstrap).map_err(|_| "bootstrap-serialize-failed")?;
        line.push(b'\n');
        Ok((grant, token, broker, line))
    }
}
