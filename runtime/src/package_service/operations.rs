impl PackageService {
    /// Development-only app install. Production packages must continue through
    /// the signed catalog path above.
    pub fn install_development_app_from_path(
        &self,
        id: &str,
        version: &str,
        path: impl AsRef<Path>,
    ) -> Result<PackageSummary, PackageError> {
        if !cfg!(debug_assertions) {
            return Err(PackageError::Invalid);
        }
        let path = path.as_ref();
        if id.len() > 64 || version.len() > 64 || !path.is_absolute() {
            return Err(PackageError::Invalid);
        }
        let bytes = fs::read(path).map_err(|_| PackageError::Invalid)?;
        let mut archive = ZipArchive::new(fs::File::open(path).map_err(|_| PackageError::Invalid)?)
            .map_err(|_| PackageError::Invalid)?;
        let mut raw_manifest = String::new();
        archive
            .by_name("manifest.json")
            .map_err(|_| PackageError::Invalid)?
            .read_to_string(&mut raw_manifest)
            .map_err(|_| PackageError::Invalid)?;
        let manifest = PackageManifest::parse(&raw_manifest).map_err(|_| PackageError::Invalid)?;
        if !matches!(manifest.kind(), PackageKind::App)
            || manifest.id() != id
            || manifest.version() != version
        {
            return Err(PackageError::Invalid);
        }
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let package = self.store.install_versioned(
            path,
            bytes.len() as u64,
            &hash,
            &manifest,
            0,
        )?;
        self.ensure_typed_grant(&package)?;
        self.store.enable(id, version)?;
        Ok(summary(
            self.store.installed(id, version)?,
            self.worker.as_ref(),
            &self.store,
        ))
    }

    pub fn list(&self) -> Result<PackageListSummary, PackageError> {
        self.list_filtered(None)
    }

    /// Manager-safe installed package view for Store. It is sourced from the
    /// Phase 5 compiled grant, never from Store catalog claims or manifests.
    pub fn store_installed_listings(&self) -> Result<Vec<InstalledListing>, PackageError> {
        let mut packages = self.store.list()?;
        packages.sort_by(|left, right| (&left.id, &left.version).cmp(&(&right.id, &right.version)));
        let grants = Self::lock(&self.typed_grants);
        Ok(packages
            .into_iter()
            .map(|package| {
                let effective_grants = match &package.manifest {
                    VersionedManifest::V1(_) => Vec::new(),
                    VersionedManifest::V2(_) => grants
                        .get(&(package.id.clone(), package.version.clone()))
                        .map(effective_grant_projections)
                        .unwrap_or_default(),
                };
                Ok(InstalledListing {
                    effective_grants,
                    id: package.id,
                    version: package.version,
                    kind: match package.manifest.kind() {
                        PackageKind::App => "app".into(),
                        PackageKind::Source => "source".into(),
                        PackageKind::Bridge => "bridge".into(),
                    },
                    enabled: package.enabled,
                    revoked: package.revoked,
                    publisher: package.manifest.publisher().to_owned(),
                })
            })
            .collect::<Result<Vec<_>, PackageError>>()?)
    }

    /// Store listings may only reference a currently trusted Package Index
    /// release. Catalog metadata never becomes package authority.
    pub fn has_catalog_release(&self, id: &str, version: &str, is_bridge: bool) -> bool {
        let state = Self::lock(&self.state);
        let (Some(trust), Some(catalog)) = (state.trust.as_ref(), state.catalog.as_ref()) else {
            return false;
        };
        let Some(entry) = trust.catalog_entry(&catalog.document, id, version) else {
            return false;
        };
        matches!(entry.manifest.kind(), PackageKind::Bridge) == is_bridge
            && trust.ensure_package_allowed(entry).is_ok()
    }

    pub fn list_filtered(
        &self,
        kind: Option<PackageKind>,
    ) -> Result<PackageListSummary, PackageError> {
        let packages = self.store.list()?;
        let state = Self::lock(&self.state);
        let packages = packages
            .into_iter()
            .filter(|package| {
                kind.as_ref()
                    .is_none_or(|kind| package.manifest.kind() == kind)
            })
            .collect::<Vec<_>>();
        let total = packages.len();
        Ok(PackageListSummary {
            packages: packages
                .into_iter()
                .take(MAX_LISTED_PACKAGES)
                .map(|package| {
                    let update_version = latest_update_version(&state, &package);
                    let mut summary = summary(package, self.worker.as_ref(), &self.store);
                    summary.update_version = update_version;
                    summary
                })
                .collect(),
            total,
            truncated: total > MAX_LISTED_PACKAGES,
        })
    }

    pub fn worker_diagnostics(&self) -> Vec<WorkerDiagnostics> {
        self.worker
            .as_ref()
            .map(|runtime| runtime.supervisor.diagnostics())
            .unwrap_or_default()
    }

    pub fn trust_summary(&self) -> TrustSummary {
        let state = Self::lock(&self.state);
        let catalog_seq = state
            .catalog
            .as_ref()
            .map_or(0, |catalog| catalog.document.sequence);
        let Some(trust) = state.trust.as_ref() else {
            return TrustSummary {
                trusted_release_keys: 0,
                revoked_release_keys: 0,
                revoked_packages: 0,
                sequence: 0,
                configured: false,
                fault_code: state.fault.clone(),
                catalog_sequence: catalog_seq,
                transition_sequence: 0,
                revocation_sequence: 0,
            };
        };
        let summary = trust.summary();
        TrustSummary {
            trusted_release_keys: summary.trusted_release_key_ids.len(),
            revoked_release_keys: summary.revoked_release_key_ids.len(),
            revoked_packages: summary.revoked_package_count,
            sequence: summary
                .catalog_sequence
                .max(summary.transition_sequence)
                .max(summary.revocation_sequence),
            configured: true,
            fault_code: state.fault.clone(),
            catalog_sequence: summary.catalog_sequence,
            transition_sequence: summary.transition_sequence,
            revocation_sequence: summary.revocation_sequence,
        }
    }

    pub fn catalog_summary(&self) -> Option<CatalogSummary> {
        Self::lock(&self.state)
            .catalog
            .as_ref()
            .map(|catalog| CatalogSummary {
                sequence: catalog.document.sequence,
                expires_at: catalog.document.expires_at.clone(),
                package_count: catalog.document.packages.len(),
            })
    }

    pub fn storage_root(&self) -> &Path {
        &self.root
    }

    /// Returns the immutable, Engine-verified package type definitions for
    /// registration in ARK.  This is intentionally a snapshot: ARK owns the
    /// database transaction and package code never receives a DB handle.
    pub fn package_type_registrations(
        &self,
    ) -> Result<Vec<ark_core::type_registry::TypeRegistration>, PackageError> {
        self.package_registrations
            .type_registrations()
            .map_err(|_| PackageError::Persistence)
    }

    pub fn configure_package_definition_dispatcher(
        &self,
        dispatcher: std::sync::Arc<crate::engine_dispatch::EngineDispatcher>,
    ) {
        *Self::lock(&self.package_definition_dispatcher) = Some(dispatcher);
    }

    pub(crate) async fn register_package_definitions(
        &self,
        dispatcher: &crate::engine_dispatch::EngineDispatcher,
    ) -> Result<(), PackageError> {
        let registrations = self.package_type_registrations()?;
        if registrations.is_empty() {
            return Ok(());
        }
        let response = dispatcher
            .dispatch(crate::engine_dispatch::DispatchRequest {
                request_id: Some("package-definition-registration".into()),
                operation: crate::engine_dispatch::Operation::Named(
                    "types.registerPackageDefinitions".into(),
                ),
                params: serde_json::json!({ "registrations": registrations }),
                client: crate::engine_dispatch::DispatchClient::default(),
            })
            .await
            .map_err(|_| PackageError::Invalid)?;
        if response.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
            Ok(())
        } else {
            Err(PackageError::Invalid)
        }
    }

    fn register_configured_package_definitions(&self) -> Result<(), PackageError> {
        let Some(dispatcher) = Self::lock(&self.package_definition_dispatcher).clone() else {
            return Ok(());
        };
        let handle =
            tokio::runtime::Handle::try_current().map_err(|_| PackageError::Persistence)?;
        tokio::task::block_in_place(|| {
            handle.block_on(self.register_package_definitions(&dispatcher))
        })
    }

    /// Resolve only an enabled, non-revoked App package for the authenticated
    /// Engine launch boundary. An omitted version selects the highest
    /// installed semver for the id.
    pub fn resolve_app(&self, id: &str, version: Option<&str>) -> Result<AppLaunch, PackageError> {
        if id.is_empty() || id.len() > 64 || id.chars().any(char::is_control) {
            return Err(PackageError::Invalid);
        }
        let mut packages = self
            .store
            .list()?
            .into_iter()
            .filter(|package| {
                package.id == id
                    && matches!(package.manifest.kind(), PackageKind::App)
                    && package.enabled
                    && !package.revoked
                    && version.is_none_or(|wanted| package.version == wanted)
            })
            .collect::<Vec<_>>();
        packages.sort_by(|left, right| {
            Version::parse(&right.version)
                .unwrap_or_else(|_| Version::new(0, 0, 0))
                .cmp(&Version::parse(&left.version).unwrap_or_else(|_| Version::new(0, 0, 0)))
        });
        let package = packages.into_iter().next().ok_or(PackageError::Invalid)?;
        let grant = self.ensure_typed_grant(&package)?;
        // Verify the immutable blob and entrypoint before minting a token.
        self.store
            .read_blob_entry(&package, package.manifest.entrypoint())?;
        Ok(AppLaunch { package, grant })
    }

    /// Compatibility alias for existing internal callers; HTTP launch lease
    /// creation is owned exclusively by the Engine API boundary.
    pub fn launch_app(&self, id: &str, version: Option<&str>) -> Result<AppLaunch, PackageError> {
        self.resolve_app(id, version)
    }

    /// Re-check package lifecycle and hash state, then read an asset from the
    /// immutable `.kspkg` blob. This is the only path used by the HTTP asset
    /// route; mutable unpacked files are never exposed.
    pub fn read_app_asset(
        &self,
        id: &str,
        version: &str,
        hash: &str,
        path: &str,
    ) -> Result<Vec<u8>, PackageError> {
        let package = self.store.installed(id, version)?;
        if !matches!(package.manifest.kind(), PackageKind::App)
            || !package.enabled
            || package.revoked
            || !package.hash.eq_ignore_ascii_case(hash)
        {
            return Err(PackageError::Invalid);
        }
        Ok(self.store.read_blob_entry(&package, path)?)
    }

    pub fn catalog_packages(
        &self,
        kind: Option<&PackageKind>,
    ) -> Result<Vec<CatalogPackageSummary>, PackageError> {
        let state = Self::lock(&self.state);
        let catalog = state
            .catalog
            .as_ref()
            .ok_or(PackageError::TrustUnavailable)?;
        Ok(catalog
            .document
            .packages
            .iter()
            .filter(|entry| kind.is_none_or(|kind| entry.manifest.kind() == kind))
            .map(|entry| CatalogPackageSummary {
                id: entry.manifest.id().to_owned(),
                version: entry.manifest.version().to_owned(),
                name: entry.manifest.name().to_owned(),
                kind: entry.manifest.kind().clone(),
                publisher: entry.manifest.publisher().to_owned(),
                archive_size: entry.size,
                revoked: state
                    .trust
                    .as_ref()
                    .is_none_or(|trust| trust.ensure_package_allowed(entry).is_err()),
            })
            .collect())
    }

    pub async fn refresh_catalog(&self) -> Result<CatalogSummary, PackageError> {
        let url = std::env::var("KOSMOS_PACKAGE_CATALOG_URL")
            .unwrap_or_else(|_| PRODUCTION_CATALOG_URL.to_string());
        let parsed = reqwest::Url::parse(&url).map_err(|_| PackageError::Invalid)?;
        if parsed.scheme() != "https"
            || parsed.username() != ""
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err(PackageError::Invalid);
        }
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|_| PackageError::Invalid)?
            .get(parsed)
            .send()
            .await
            .map_err(|_| PackageError::Invalid)?;
        if !response.status().is_success() {
            return Err(PackageError::Invalid);
        }
        let body = response.bytes().await.map_err(|_| PackageError::Invalid)?;
        if body.len() > MAX_ENVELOPE as usize {
            return Err(PackageError::Invalid);
        }
        let envelope: Envelope =
            serde_json::from_slice(&body).map_err(|_| PackageError::Invalid)?;
        let bytes = STANDARD
            .decode(envelope.bytes)
            .map_err(|_| PackageError::Invalid)?;
        self.apply_catalog(bytes, envelope.signatures)
    }

    pub fn apply_catalog(
        &self,
        bytes: impl AsRef<[u8]>,
        signatures: SignatureSet,
    ) -> Result<CatalogSummary, PackageError> {
        let _mutation = Self::lock(&self.mutation);
        let bytes = bytes.as_ref();
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_catalog(bytes, signatures.clone())?;
        trust.apply_catalog(bytes, signatures.clone())?;
        if self.persist("catalog.json", bytes, &signatures).is_err() {
            Self::disable_trust(&mut state);
            return Err(PackageError::Persistence);
        }
        state.catalog = Some(CatalogState {
            document: verified.document.clone(),
            signatures,
        });
        state.fault = None;
        Ok(CatalogSummary {
            sequence: verified.document.sequence,
            expires_at: verified.document.expires_at,
            package_count: verified.document.packages.len(),
        })
    }

    pub fn apply_transition(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_key_transition(bytes, signatures.clone())?;
        if trust
            .apply_key_transition(bytes, signatures.clone())
            .is_err()
        {
            Self::disable_trust(&mut state);
            return Err(PackageError::TrustUnavailable);
        }
        if self
            .persist(
                &format!("transition-{:020}.json", verified.document.sequence),
                bytes,
                &signatures,
            )
            .is_err()
        {
            Self::disable_trust(&mut state);
            return Err(PackageError::Persistence);
        }
        Ok(())
    }

    pub fn apply_revocations(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        if bytes.len() > MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
        let verified = trust.verify_revocations(bytes, signatures.clone())?;
        if trust.apply_revocations(bytes, signatures.clone()).is_err() {
            Self::disable_trust(&mut state);
            return Err(PackageError::TrustUnavailable);
        }
        let revoked = verified.document.revoked_packages.iter().map(|package| {
            (
                package.id.as_str(),
                package.version.as_str(),
                package.sha256.as_str(),
            )
        });
        if self.store.reconcile_revocations(revoked).is_err() {
            Self::disable_trust(&mut state);
            return Err(PackageError::TrustUnavailable);
        }
        if self
            .persist(
                &format!("revocation-{:020}.json", verified.document.sequence),
                bytes,
                &signatures,
            )
            .is_err()
        {
            Self::disable_trust(&mut state);
            return Err(PackageError::Persistence);
        }
        let catalog_untrusted = state.catalog.as_ref().is_some_and(|catalog| {
            catalog
                .signatures
                .signatures
                .iter()
                .all(|signature| !trust.is_release_key_trusted(&signature.key_id))
        });
        if catalog_untrusted {
            Self::disable_catalog(&mut state);
        }
        Ok(())
    }

    pub async fn apply_revocations_with_worker_stop(
        &self,
        bytes: &[u8],
        signatures: SignatureSet,
    ) -> Result<(), PackageError> {
        let targets = {
            let state = Self::lock(&self.state);
            let trust = state.trust.as_ref().ok_or(PackageError::TrustUnavailable)?;
            trust
                .verify_revocations(bytes, signatures.clone())?
                .document
                .revoked_packages
                .into_iter()
                .map(|package| (package.id, package.version))
                .collect::<Vec<_>>()
        };
        if let Some(worker) = self.worker.as_ref() {
            let _worker_mutation = worker.mutation.lock().await;
            for (id, version) in &targets {
                worker
                    .supervisor
                    .stop(id, version)
                    .await
                    .map_err(|_| PackageError::Persistence)?;
            }
            return self.apply_revocations(bytes, signatures);
        }
        self.apply_revocations(bytes, signatures)
    }

    pub fn install_from_path(
        &self,
        id: &str,
        version: &str,
        path: impl AsRef<Path>,
    ) -> Result<PackageSummary, PackageError> {
        let _mutation = Self::lock(&self.mutation);
        let path = path.as_ref();
        if id.len() > 64
            || version.len() > 64
            || !path.is_absolute()
            || path.as_os_str().len() > 4096
        {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        let entry = Self::current_entry(&mut state, id, version)?;
        let sequence = state
            .catalog
            .as_ref()
            .map(|catalog| catalog.document.sequence)
            .ok_or(PackageError::TrustUnavailable)?;
        let definition_documents = match &entry.manifest {
            VersionedManifest::V2(manifest) if !manifest.data.defines.is_empty() => {
                let documents = definition_documents_from_archive(path, manifest)?;
                self.package_registrations
                    .validate_manifest(manifest, &documents)
                    .map_err(|_| PackageError::Invalid)?;
                Some((manifest.clone(), documents))
            }
            _ => None,
        };
        let definition_snapshot = definition_documents
            .as_ref()
            .map(|_| self.package_registrations.snapshot())
            .transpose()
            .map_err(|_| PackageError::Persistence)?;
        let previous = self.store.installed(id, version).ok();
        if self.worker.as_ref().is_some_and(|worker| {
            !matches!(
                worker.supervisor.health(id, version).state,
                WorkerState::Stopped
            )
        }) {
            // Synchronous callers cannot safely stop a worker. The async
            // wrapper below owns that lifecycle transition; fail closed for
            // any direct caller that bypasses it.
            return Err(PackageError::Persistence);
        }
        let package = self.store.install_versioned(
            path,
            entry.size,
            &entry.sha256,
            &entry.manifest,
            sequence,
        )?;
        if let Some((manifest, documents)) = definition_documents {
            if let Err(error) = self
                .package_registrations
                .register_manifest(&manifest, &documents)
            {
                let rollback = self.restore_install_after_failure(
                    definition_snapshot.as_deref(),
                    previous.as_ref(),
                    &package.id,
                    &package.version,
                );
                if let Err(rollback) = rollback {
                    return Err(rollback);
                }
                return Err(match error {
                    crate::package_registration::RegistrationError::Persistence => {
                        PackageError::Persistence
                    }
                    _ => PackageError::Invalid,
                });
            }
            if let Err(error) = self.refresh_typed_registry() {
                self.restore_install_after_failure(
                    definition_snapshot.as_deref(),
                    previous.as_ref(),
                    &package.id,
                    &package.version,
                )?;
                return Err(error);
            }
            if let Err(error) = self.ensure_typed_grant(&package) {
                self.restore_install_after_failure(
                    definition_snapshot.as_deref(),
                    previous.as_ref(),
                    &package.id,
                    &package.version,
                )?;
                return Err(error);
            }
            if let Err(error) = self.register_configured_package_definitions() {
                self.restore_install_after_failure(
                    definition_snapshot.as_deref(),
                    previous.as_ref(),
                    &package.id,
                    &package.version,
                )?;
                return Err(error);
            }
        } else if let Err(error) = self.ensure_typed_grant(&package) {
            self.restore_install_after_failure(
                None,
                previous.as_ref(),
                &package.id,
                &package.version,
            )?;
            return Err(error);
        }
        Ok(summary(package, self.worker.as_ref(), &self.store))
    }

    /// Replace a package only after its worker has been stopped and its grant
    /// revoked. If installation fails, the exact prior record is restored by
    /// `install_from_path`; an enabled worker is restarted only after that
    /// record is verified byte-for-byte in the store.
    pub async fn install_from_path_with_worker_stop(
        &self,
        id: &str,
        version: &str,
        path: impl AsRef<Path>,
    ) -> Result<PackageSummary, PackageError> {
        let previous = self.store.installed(id, version).ok();
        let worker_guard = if let Some(worker) = self.worker.as_ref() {
            Some(worker.mutation.lock().await)
        } else {
            None
        };
        let worker_was_live = self.worker.as_ref().is_some_and(|worker| {
            !matches!(
                worker.supervisor.health(id, version).state,
                WorkerState::Stopped
            )
        });
        let should_restart = worker_was_live
            && previous
                .as_ref()
                .is_some_and(|package| package.enabled && !package.revoked);
        let definition_snapshot = should_restart
            .then(|| self.package_registrations.snapshot())
            .transpose()
            .map_err(|_| PackageError::Persistence)?;
        if worker_was_live {
            let worker = self.worker.as_ref().ok_or(PackageError::Persistence)?;
            if worker.supervisor.stop(id, version).await.is_err() {
                return Err(PackageError::Persistence);
            }
        }

        let result = self.install_from_path(id, version, path);
        drop(worker_guard);
        match result {
            Ok(summary) if !should_restart => Ok(summary),
            Ok(_) => match self.set_enabled(id, version, true).await {
                Ok(summary) => Ok(summary),
                Err(error) => {
                    if let Some(worker) = self.worker.as_ref() {
                        let _ = worker.supervisor.stop(id, version).await;
                    }
                    let Some(previous) = previous.as_ref() else {
                        return Err(PackageError::Persistence);
                    };
                    let restored = self.restore_install_after_failure(
                        definition_snapshot.as_deref(),
                        Some(previous),
                        id,
                        version,
                    );
                    if restored.is_err()
                        || self.store.installed(id, version).ok().as_ref() != Some(previous)
                    {
                        return Err(PackageError::Persistence);
                    }
                    Err(error)
                }
            },
            Err(error) => {
                if !should_restart {
                    return Err(error);
                }
                let Some(previous) = previous.as_ref() else {
                    return Err(PackageError::Persistence);
                };
                if !restored_package_record_matches(&self.store, previous, id, version) {
                    if let Some(worker) = self.worker.as_ref() {
                        let _ = worker.supervisor.stop(id, version).await;
                    }
                    let _ = self.disable(id, version);
                    return Err(PackageError::Persistence);
                }
                if self.set_enabled(id, version, true).await.is_err() {
                    if let Some(worker) = self.worker.as_ref() {
                        let _ = worker.supervisor.stop(id, version).await;
                    }
                    return Err(PackageError::Persistence);
                }
                Err(error)
            }
        }
    }

    pub async fn install_from_catalog(
        &self,
        id: &str,
        version: &str,
    ) -> Result<PackageSummary, PackageError> {
        let (url, expected_size) = {
            let mut state = Self::lock(&self.state);
            let entry = Self::current_entry(&mut state, id, version)?;
            (entry.archive_url, entry.size)
        };
        let parsed = reqwest::Url::parse(&url).map_err(|_| PackageError::Invalid)?;
        if parsed.scheme() != "https"
            || parsed.username() != ""
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err(PackageError::Invalid);
        }
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|_| PackageError::Invalid)?
            .get(parsed)
            .send()
            .await
            .map_err(|_| PackageError::Invalid)?;
        if !response.status().is_success() {
            return Err(PackageError::Invalid);
        }
        if response
            .content_length()
            .is_some_and(|size| size != expected_size)
        {
            return Err(PackageError::Invalid);
        }
        let bytes = response.bytes().await.map_err(|_| PackageError::Invalid)?;
        if bytes.len() as u64 != expected_size {
            return Err(PackageError::Invalid);
        }
        let path = self.root.join(format!(".download-{id}-{version}.kspkg"));
        fs::write(&path, &bytes).map_err(|_| PackageError::Persistence)?;
        let result = self
            .install_from_path_with_worker_stop(id, version, &path)
            .await;
        let _ = fs::remove_file(path);
        result
    }

    pub fn enable(&self, id: &str, version: &str) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        let mut state = Self::lock(&self.state);
        let entry = Self::current_entry(&mut state, id, version)?;
        let installed = self
            .store
            .list()?
            .into_iter()
            .find(|package| package.id == id && package.version == version)
            .ok_or(PackageError::Invalid)?;
        if !installed.hash.eq_ignore_ascii_case(&entry.sha256) {
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
        let typed_bound = self.ensure_typed_grant(&installed)?;
        if matches!(installed.manifest.kind(), PackageKind::App) {
            if enabled {
                self.enable(id, version)?;
            } else {
                self.disable(id, version)?;
            }
            return Ok(summary(
                self.store.installed(id, version)?,
                self.worker.as_ref(),
                &self.store,
            ));
        }
        let worker = self.worker.as_ref().ok_or(PackageError::Invalid)?;
        let _worker_mutation = worker.mutation.lock().await;
        if !enabled {
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
            self.disable(id, version)?;
            return Ok(summary(
                self.store.installed(id, version)?,
                self.worker.as_ref(),
                &self.store,
            ));
        }
        let launch = {
            let _mutation = Self::lock(&self.mutation);
            let mut state = Self::lock(&self.state);
            let entry = Self::current_entry(&mut state, id, version)?;
            let installed = self.store.installed(id, version)?;
            if installed.revoked || !installed.hash.eq_ignore_ascii_case(&entry.sha256) {
                return Err(PackageError::Invalid);
            }
            let executable = self.store.immutable_entrypoint(&installed)?;
            let bridge_config = if matches!(installed.manifest.kind(), PackageKind::Bridge) {
                let config = Self::lock(&self.bridge_configs)
                    .configs
                    .get(&Self::bridge_key(id, version))
                    .cloned()
                    .ok_or(PackageError::Invalid)?;
                let state_root = self.root.join("bridge-state").join(id).join(version);
                Some(BridgeWorkerConfig {
                    vault_root: config.vault_root,
                    state_root: state_root.to_string_lossy().into_owned(),
                    selected_types: config.selected_types,
                    editable_fields: config.editable_fields,
                    readonly_fields: config.readonly_fields,
                })
            } else {
                None
            };
            let roots = if matches!(installed.manifest.kind(), PackageKind::Bridge) {
                self.bridge_roots(id, version)?
            } else {
                worker.roots.clone()
            };
            let integration = self.integration_launch_config(&installed)?;
            (
                installed.manifest.common_manifest(),
                installed.hash,
                executable,
                roots,
                bridge_config,
                integration,
            )
        };
        worker
            .supervisor
            .bind_typed_launch(id, version, &worker.correlation_id, 1, typed_bound)
            .map_err(|_| {
                worker.supervisor.revoke_typed_launch(id, version);
                PackageError::Invalid
            })?;
        worker
            .supervisor
            .start(
                &launch.0,
                launch.2,
                launch.1,
                &launch.3,
                worker.correlation_id.clone(),
                launch.4,
                launch.5,
            )
            .await
            .map_err(|_| {
                worker.supervisor.revoke_typed_launch(id, version);
                PackageError::Invalid
            })?;
        if self.store.enable_worker(id, version).is_err() {
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
            worker.supervisor.revoke_typed_launch(id, version);
            return Err(PackageError::Persistence);
        }
        if !worker.supervisor.activate(id, version) {
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
            worker.supervisor.revoke_typed_launch(id, version);
            let _ = self.store.disable(id, version);
            return Err(PackageError::Invalid);
        }
        Ok(summary(
            self.store.installed(id, version)?,
            self.worker.as_ref(),
            &self.store,
        ))
    }

    pub fn disable(&self, id: &str, version: &str) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        if let Some(worker) = self.worker.as_ref() {
            worker.supervisor.revoke_typed_launch(id, version);
        }
        self.store.disable(id, version)?;
        Ok(())
    }

    pub fn uninstall(&self, id: &str, version: &str) -> Result<(), PackageError> {
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
