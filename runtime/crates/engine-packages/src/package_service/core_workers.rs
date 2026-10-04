impl PackageService {
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
    pub fn validate_legacy_grants(&self, source_ids: &[String]) -> Result<(), PackageError> {
        let source_ids = source_ids.iter().map(String::as_str).collect::<Vec<_>>();
        self.grants
            .validate_legacy_records(&source_ids)
            .map_err(|_| PackageError::Persistence)
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
                    if worker
                        .supervisor
                        .health(&package.id, &package.version)
                        .state
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
        let size = retry_io(|| fs::metadata(&archive))
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
}
