impl PackageService {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, PackageError> {
        let root = data_dir.as_ref().join("packages");
        fs::create_dir_all(&root).map_err(|_| PackageError::Persistence)?;
        let trust = match (
            option_env!("KOSMOS_PACKAGE_ROOT_KEY_JSON"),
            option_env!("KOSMOS_PACKAGE_RELEASE_KEYS_JSON"),
        ) {
            (Some(root_json), Some(releases)) => serde_json::from_str::<TrustedKey>(root_json)
                .ok()
                .and_then(|root_key| {
                    serde_json::from_str::<Vec<TrustedKey>>(releases)
                        .ok()
                        .and_then(|keys| TrustStore::new(root_key, keys).ok())
                }),
            _ => production_trust(),
        };
        Self::from_parts(root, trust)
    }
    #[cfg(test)]
    pub fn open_with_trust(
        data_dir: impl AsRef<Path>,
        trust: TrustStore,
    ) -> Result<Self, PackageError> {
        Self::from_parts(data_dir.as_ref().join("packages"), Some(trust))
    }

    fn from_parts(root: PathBuf, trust: Option<TrustStore>) -> Result<Self, PackageError> {
        let unavailable = trust.is_none();
        let data_dir = root
            .parent()
            .ok_or(PackageError::Persistence)?
            .to_path_buf();
        let bridge_configs = read_bridge_configs(&root);
        let store = std::sync::Arc::new(PackageStore::new(&root)?);
        let package_registrations = PackageRegistrationRegistry::open(root.join("definitions"))
            .map_err(|_| PackageError::Persistence)?;
        // Re-validate every installed package against its immutable blob on
        // startup. Invalid/tampered package definitions fail closed, while
        // definitions from an uninstall remain in the registry file.
        for package in store.list()? {
            let VersionedManifest::V2(manifest) = &package.manifest else {
                let _ = store.disable(&package.id, &package.version);
                continue;
            };
            if manifest.data.defines.is_empty() {
                continue;
            }
            let Ok(documents) = definition_documents_from_store(&store, &package) else {
                let _ = store.disable(&package.id, &package.version);
                continue;
            };
            if package_registrations
                .register_manifest(manifest, &documents)
                .is_err()
            {
                let _ = store.disable(&package.id, &package.version);
            }
        }
        let mut typed_registry = canonical_registry_snapshot()?;
        typed_registry.types.extend(
            package_registrations
                .registered_types()
                .map_err(|_| PackageError::Persistence)?,
        );
        let service = Self {
            store,
            root,
            state: Mutex::new(State {
                trust,
                fault: unavailable.then(|| "package_trust_unavailable".into()),
                catalog: None,
            }),
            mutation: Mutex::new(()),
            worker: None,
            bridge_configs: Mutex::new(bridge_configs),
            typed_grants: Mutex::new(HashMap::new()),
            package_registrations,
            package_definition_dispatcher: Mutex::new(None),
            typed_registry: Mutex::new(typed_registry),
            grants: std::sync::Arc::new(GrantAuthorityRegistry::with_data_dir(data_dir)),
        };
        // A stale installed v2 contract must not brick Engine startup. Invalid
        // packages stay installed but disabled and cannot be enabled/launched.
        let _ = service.rebuild_typed_grants();
        service.replay();
        Ok(service)
    }

    pub fn build_engine_snapshot(
        &self,
        package_id: &str,
        source: &str,
    ) -> Result<Vec<PackageSnapshotFile>, PackageError> {
        if package_id.is_empty()
            || !package_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            || source != "installed" && source != "auto"
        {
            return Err(PackageError::Invalid);
        }
        let package = self
            .store
            .list()?
            .into_iter()
            .find(|item| item.id == package_id && !item.revoked && item.enabled)
            .ok_or(PackageError::Invalid)?;
        let root = self
            .root
            .join("unpacked")
            .join(&package.id)
            .join(&package.version)
            .join(&package.hash);
        let config =
            BrokerConfig::new(std::iter::empty::<&str>(), vec![self.root.join("unpacked")])
                .map_err(|_| PackageError::Invalid)?;
        read_snapshot_tree(&config, &root).map_err(|_| PackageError::Invalid)
    }
    pub fn engine_snapshot_identity(
        &self,
        package_id: &str,
        _source: &str,
    ) -> Result<(String, u64, u64), PackageError> {
        let package = self
            .store
            .list()?
            .into_iter()
            .find(|item| item.id == package_id && !item.revoked && item.enabled)
            .ok_or(PackageError::Invalid)?;
        let root = fs::canonicalize(
            self.root
                .join("unpacked")
                .join(&package.id)
                .join(&package.version)
                .join(&package.hash),
        )
        .map_err(|_| PackageError::Invalid)?;
        let metadata = fs::metadata(&root).map_err(|_| PackageError::Invalid)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            return Ok((
                root.to_string_lossy().into_owned(),
                metadata.dev(),
                metadata.ino(),
            ));
        }
        #[cfg(not(unix))]
        {
            let _ = (_source, metadata);
            Err(PackageError::Invalid)
        }
    }
    pub fn build_engine_grant_snapshot(
        &self,
        extension_id: &str,
        root: &Path,
        identity_dev: u64,
        identity_ino: u64,
        exact_file: bool,
    ) -> Result<Vec<PackageSnapshotFile>, PackageError> {
        if extension_id.is_empty() || !root.is_absolute() {
            return Err(PackageError::Invalid);
        }
        let configured_parent = self.root.parent().ok_or(PackageError::Invalid)?;
        let canonical = fs::canonicalize(root).map_err(|_| PackageError::Invalid)?;
        let configured_parent =
            fs::canonicalize(configured_parent).map_err(|_| PackageError::Invalid)?;
        if !(canonical == configured_parent || canonical.starts_with(&configured_parent)) {
            return Err(PackageError::Invalid);
        }
        let metadata = fs::metadata(&canonical).map_err(|_| PackageError::Invalid)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.dev() != identity_dev || metadata.ino() != identity_ino {
                return Err(PackageError::Invalid);
            }
        }
        #[cfg(not(unix))]
        let _ = (identity_dev, identity_ino, metadata);
        let snapshot_root = if exact_file {
            canonical.parent().ok_or(PackageError::Invalid)?
        } else {
            &canonical
        };
        let config = BrokerConfig::new(std::iter::empty::<&str>(), vec![configured_parent])
            .map_err(|_| PackageError::Invalid)?;
        let mut files =
            read_snapshot_tree(&config, snapshot_root).map_err(|_| PackageError::Invalid)?;
        if exact_file {
            let filename = canonical
                .file_name()
                .ok_or(PackageError::Invalid)?
                .to_string_lossy();
            files.retain(|file| file.path == filename);
        }
        if files.is_empty() {
            return Err(PackageError::Invalid);
        }
        Ok(files)
    }
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

    pub fn configure_workers(
        &mut self,
        supervisor: PackageWorkerSupervisor,
        roots: Vec<PathBuf>,
        correlation_id: String,
    ) {
        supervisor.bind_grant_authority(self.grants.clone());
        self.worker = Some(WorkerRuntime {
            supervisor,
            roots,
            correlation_id,
            mutation: tokio::sync::Mutex::new(()),
        });
        if let Some(worker) = self.worker.as_ref() {
            worker.supervisor.bind_store(self.store.clone());
        }
    }

    pub fn grant_authority(&self) -> std::sync::Arc<GrantAuthorityRegistry> {
        self.grants.clone()
    }

    pub fn revoke_legacy_grants(&self, source_ids: &[String]) -> Result<usize, PackageError> {
        let source_ids = source_ids.iter().map(String::as_str).collect::<Vec<_>>();
        self.grants
            .revoke_legacy_records(&source_ids)
            .map_err(|error| match error {
                crate::grant_authority::GrantError::Invalid => PackageError::Invalid,
                _ => PackageError::Persistence,
            })
    }

    pub async fn restore_enabled_workers(&self) -> Result<(), PackageError> {
        let packages = self.store.list()?;
        for package in packages.into_iter().filter(|package| {
            package.enabled && !package.revoked && package.manifest.worker_entrypoint().is_some()
        }) {
            if let Err(error) = self.set_enabled(&package.id, &package.version, true).await {
                tracing::warn!(
                    target: "package_worker",
                    package_id = %package.id,
                    version = %package.version,
                    %error,
                    "enabled worker restore failed"
                );
                if let Some(worker) = self.worker.as_ref() {
                    if worker.supervisor.health(&package.id, &package.version).state
                        != WorkerState::Stopped
                    {
                        worker
                            .supervisor
                            .stop(&package.id, &package.version)
                            .await
                            .map_err(|_| PackageError::Persistence)?;
                    }
                }
                self.disable(&package.id, &package.version)?;
            }
        }
        Ok(())
    }

    /// Install the host-compiled v2 grant that will be bound to the next
    /// authenticated worker launch. This is intentionally private to the
    /// Engine: the registry snapshot is canonical ARK data, never renderer
    /// input.
    fn register_typed_manifest(
        &self,
        manifest: ManifestV2,
        registry: &RegistrySnapshot,
        manifest_digest: &str,
    ) -> Result<(), PackageError> {
        manifest.validate().map_err(|_| PackageError::Invalid)?;
        let grant = compile_manifest_v2(&manifest, registry, manifest_digest)
            .map_err(|_| PackageError::Invalid)?;
        Self::lock(&self.typed_grants).insert((manifest.id, manifest.version), grant);
        Ok(())
    }

    fn rebuild_typed_grants(&self) -> Result<(), PackageError> {
        let mut grants = HashMap::new();
        let registry = Self::lock(&self.typed_registry);
        for package in self.store.list()? {
            let VersionedManifest::V2(manifest) = package.manifest else {
                let _ = self.store.disable(&package.id, &package.version);
                continue;
            };
            let Ok(digest) = manifest_digest(&manifest) else {
                let _ = self.store.disable(&package.id, &package.version);
                continue;
            };
            let Ok(grant) = compile_manifest_v2(&manifest, &registry, &digest) else {
                let _ = self.store.disable(&package.id, &package.version);
                continue;
            };
            grants.insert((manifest.id, manifest.version), grant);
        }
        *Self::lock(&self.typed_grants) = grants;
        Ok(())
    }

    fn refresh_typed_registry(&self) -> Result<(), PackageError> {
        let mut registry = canonical_registry_snapshot()?;
        registry.types.extend(
            self.package_registrations
                .registered_types()
                .map_err(|_| PackageError::Persistence)?,
        );
        *Self::lock(&self.typed_registry) = registry;
        Ok(())
    }

    fn restore_install_after_failure(
        &self,
        definition_snapshot: Option<&[u8]>,
        previous: Option<&InstalledPackage>,
        id: &str,
        version: &str,
    ) -> Result<(), PackageError> {
        if let Some(snapshot) = definition_snapshot {
            self.package_registrations
                .restore(snapshot)
                .map_err(|_| PackageError::Persistence)?;
        }
        self.store.uninstall(id, version)?;
        let Some(previous) = previous else {
            self.refresh_typed_registry()?;
            return self.rebuild_typed_grants();
        };
        let archive = self
            .root
            .join("blobs")
            .join(format!("{}.kspkg", previous.hash));
        let size = fs::metadata(&archive)
            .map_err(|_| PackageError::Persistence)?
            .len();
        self.store.install_versioned(
            &archive,
            size,
            &previous.hash,
            &previous.manifest,
            previous.catalog_sequence,
        )?;
        self.store.restore_record(previous)?;
        self.refresh_typed_registry()?;
        self.rebuild_typed_grants()
    }

    fn ensure_typed_grant(&self, package: &InstalledPackage) -> Result<LaunchGrant, PackageError> {
        let VersionedManifest::V2(manifest) = &package.manifest else {
            return Err(PackageError::Invalid);
        };
        let digest = manifest_digest(manifest)?;
        let registry = Self::lock(&self.typed_registry);
        self.register_typed_manifest(manifest.clone(), &registry, &digest)?;
        Self::lock(&self.typed_grants)
            .get(&(package.id.clone(), package.version.clone()))
            .cloned()
            .ok_or(PackageError::Invalid)
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn disable_trust(state: &mut State) {
        state.trust = None;
        state.catalog = None;
        state.fault = Some("package_trust_unavailable".into());
    }

    fn disable_catalog(state: &mut State) {
        state.catalog = None;
        state.fault = Some("catalog_unavailable".into());
    }

    fn replay(&self) {
        let _mutation = Self::lock(&self.mutation);
        let mut state = Self::lock(&self.state);
        if state.trust.is_none() {
            return;
        }
        for path in self.document_paths("transition-") {
            if self.replay_transition(&mut state, &path).is_err() {
                Self::disable_trust(&mut state);
                return;
            }
        }
        for path in self.document_paths("revocation-") {
            if self.replay_revocation(&mut state, &path).is_err() {
                Self::disable_trust(&mut state);
                return;
            }
        }
        let path = self.root.join("catalog.json");
        if path.exists() && self.replay_catalog(&mut state, &path).is_err() {
            // A stale/revoked/corrupt catalog must never erase configured trust.
            Self::disable_catalog(&mut state);
        }
    }

    fn document_paths(&self, prefix: &str) -> Vec<PathBuf> {
        let mut paths = fs::read_dir(&self.root)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".json"))
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    fn read_env(path: &Path) -> Result<(Vec<u8>, SignatureSet), PackageError> {
        let metadata = fs::metadata(path).map_err(|_| PackageError::Persistence)?;
        if !metadata.is_file() || metadata.len() > MAX_ENVELOPE {
            return Err(PackageError::Invalid);
        }
        let env: Envelope =
            serde_json::from_slice(&fs::read(path).map_err(|_| PackageError::Persistence)?)
                .map_err(|_| PackageError::Persistence)?;
        let bytes = STANDARD
            .decode(env.bytes)
            .map_err(|_| PackageError::Persistence)?;
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        Ok((bytes, env.signatures))
    }

    fn replay_transition(&self, state: &mut State, path: &Path) -> Result<(), PackageError> {
        let (bytes, signatures) = Self::read_env(path)?;
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        trust.verify_key_transition(&bytes, signatures.clone())?;
        trust.apply_key_transition(&bytes, signatures)?;
        Ok(())
    }

    fn replay_revocation(&self, state: &mut State, path: &Path) -> Result<(), PackageError> {
        let (bytes, signatures) = Self::read_env(path)?;
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_revocations(&bytes, signatures.clone())?;
        trust.apply_revocations(&bytes, signatures)?;
        self.store
            .reconcile_revocations(verified.document.revoked_packages.iter().map(|package| {
                (
                    package.id.as_str(),
                    package.version.as_str(),
                    package.sha256.as_str(),
                )
            }))?;
        Ok(())
    }

    fn replay_catalog(&self, state: &mut State, path: &Path) -> Result<(), PackageError> {
        let (bytes, signatures) = Self::read_env(path)?;
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_catalog(&bytes, signatures.clone())?;
        trust.apply_catalog(&bytes, signatures.clone())?;
        state.catalog = Some(CatalogState {
            document: verified.document,
            signatures,
        });
        state.fault = None;
        Ok(())
    }

    fn persist(
        &self,
        name: &str,
        bytes: &[u8],
        signatures: &SignatureSet,
    ) -> Result<(), PackageError> {
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        let envelope = Envelope {
            bytes: STANDARD.encode(bytes),
            signatures: signatures.clone(),
        };
        if serde_json::to_vec(&envelope)
            .map_err(|_| PackageError::Persistence)?
            .len() as u64
            > MAX_ENVELOPE
        {
            return Err(PackageError::Invalid);
        }
        write_owner_only_json(&self.root.join(name), &envelope)
            .map_err(|_| PackageError::Persistence)
    }

    fn current_entry(
        state: &mut State,
        id: &str,
        version: &str,
    ) -> Result<CatalogEntry, PackageError> {
        let Some(trust) = state.trust.as_ref() else {
            return Err(PackageError::TrustUnavailable);
        };
        let Some(catalog) = state.catalog.as_ref() else {
            return Err(PackageError::TrustUnavailable);
        };
        let expiry = DateTime::parse_from_rfc3339(&catalog.document.expires_at)
            .map_err(|_| PackageError::Invalid)?
            .with_timezone(&Utc);
        if expiry <= Utc::now()
            || !catalog
                .signatures
                .signatures
                .iter()
                .any(|signature| trust.is_release_key_trusted(&signature.key_id))
        {
            Self::disable_catalog(state);
            return Err(PackageError::TrustUnavailable);
        }
        let entry = trust
            .catalog_entry(&catalog.document, id, version)
            .cloned()
            .ok_or(PackageError::Invalid)?;
        trust.ensure_package_allowed(&entry)?;
        VersionReq::parse(entry.manifest.engine_api())
            .map_err(|_| PackageError::Invalid)?
            .matches(&Version::new(
                API_VERSION_CURRENT.major.into(),
                API_VERSION_CURRENT.minor.into(),
                API_VERSION_CURRENT.patch.into(),
            ))
            .then_some(entry)
            .ok_or(PackageError::Invalid)
    }
}
