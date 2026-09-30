impl PackageService {
pub async fn invoke_worker_operation(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, PackageError> {
        let worker = self.worker.as_ref().ok_or(PackageError::Invalid)?;
        let _worker_mutation = worker.mutation.lock().await;
        let mut packages = self
            .store
            .list()?
            .into_iter()
            .filter(|package| package.enabled && !package.revoked)
            .filter(|package| {
                package.manifest.permissions().iter().any(|permission| {
                    permission.capability == "worker.invoke"
                        && permission.scopes.iter().any(|scope| {
                            scope == operation
                                || scope.strip_suffix(".*").is_some_and(|prefix| {
                                    operation.starts_with(&format!("{prefix}."))
                                })
                        })
                })
            });
        let package = packages.next().ok_or(PackageError::Invalid)?;
        if packages.next().is_some() {
            return Err(PackageError::Invalid);
        }
        worker
            .supervisor
            .invoke(&package.id, &package.version, operation, params)
            .await
            .inspect_err(|error| {
                tracing::warn!(
                    target: "package_worker",
                    package_id = %package.id,
                    version = %package.version,
                    operation,
                    error,
                    "worker invocation failed"
                );
            })
            .map_err(PackageError::Worker)
    }

    /// Development-only install from a local archive. Production packages
    /// must continue through the signed catalog path above. Any package kind
    /// is accepted so locally built sources and bridges can be exercised
    /// without a catalog release.
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
        if manifest.id() != id || manifest.version() != version {
            return Err(PackageError::Invalid);
        }
        let definition_documents = match &manifest {
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
            return Err(PackageError::Persistence);
        }
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let package = self.store.install_versioned(
            path,
            bytes.len() as u64,
            &hash,
            &manifest,
            0,
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
        if package.manifest.worker_entrypoint().is_none() {
            self.store.enable(id, version)?;
        }
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
    pub fn has_catalog_release(
        &self,
        id: &str,
        version: &str,
        expected_kind: &crate::package_manifest::PackageKind,
    ) -> bool {
        let state = Self::lock(&self.state);
        let (Some(trust), Some(catalog)) = (state.trust.as_ref(), state.catalog.as_ref()) else {
            return false;
        };
        let Some(entry) = trust.catalog_entry(&catalog.document, id, version) else {
            return false;
        };
        entry.manifest.kind() == expected_kind
            && trust.ensure_package_allowed(entry).is_ok()
    }

    /// Read-only projection of the declared permission contract for a package
    /// version. The signed catalog entry is preferred because it is what a
    /// pending install would deliver; the verified installed manifest covers
    /// versions that have already left the catalog.
    pub fn disclosure(&self, id: &str, version: &str) -> Result<PackageDisclosure, PackageError> {
        if id.is_empty() || version.is_empty() {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        if let Ok(entry) = Self::current_entry(&mut state, id, version) {
            return Ok(package_disclosure(&entry.manifest));
        }
        drop(state);
        if let Ok(package) = self.store.installed(id, version) {
            if !package.revoked {
                return Ok(package_disclosure(&package.manifest));
            }
        }
        Err(PackageError::Invalid)
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

    /// Weak back-reference: the dispatcher's dispatch closure captures an
    /// `Arc<PackageService>`, so a strong reference here would form a cycle
    /// that keeps both (and every component in the closure) alive until
    /// process exit (KOS-270). A dropped dispatcher simply fails to upgrade.
    pub fn configure_package_definition_dispatcher(
        &self,
        dispatcher: std::sync::Arc<crate::engine_dispatch::EngineDispatcher>,
    ) {
        *Self::lock(&self.package_definition_dispatcher) =
            Some(std::sync::Arc::downgrade(&dispatcher));
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
        let Some(dispatcher) = Self::lock(&self.package_definition_dispatcher)
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
        else {
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

    /// Verify an installed App without changing package, worker, grant, or
    /// catalog state. This is the migration preflight boundary.
    pub fn verify_installed_app(
        &self,
        id: &str,
        version: &str,
        hash: &str,
        catalog_sequence: u64,
    ) -> Result<VerifiedReplacement, PackageError> {
        if id.is_empty() || version.is_empty() || hash.is_empty() {
            return Err(PackageError::Invalid);
        }
        let _mutation = Self::lock(&self.mutation);
        let mut state = Self::lock(&self.state);
        let package = self.store.installed(id, version)?;
        if !matches!(package.manifest.kind(), PackageKind::App)
            || package.revoked
            || package.catalog_sequence != catalog_sequence
            || !package.hash.eq_ignore_ascii_case(hash)
            || !matches!(
                &package.manifest,
                VersionedManifest::V2(manifest) if manifest.supports_current_platform()
            )
        {
            return Err(PackageError::Invalid);
        }
        let entry = Self::current_entry(&mut state, id, version)?;
        if entry.manifest != package.manifest || !entry.sha256.eq_ignore_ascii_case(&package.hash) {
            return Err(PackageError::Invalid);
        }
        // `installed` verifies manifest.json; these explicit reads also prove
        // the immutable archive and the selected entrypoint before migration.
        self.store.read_blob_entry(&package, "manifest.json")?;
        self.store
            .immutable_asset_path(&package, package.manifest.entrypoint())?;
        Ok(VerifiedReplacement {
            id: package.id,
            version: package.version,
            hash: package.hash,
            catalog_sequence: package.catalog_sequence,
        })
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
        let url = std::env::var("MUNDUS_PACKAGE_CATALOG_URL")
            .unwrap_or_else(|_| PRODUCTION_CATALOG_URL.to_string());
        let parsed = reqwest::Url::parse(&url).map_err(|_| PackageError::Invalid)?;
        if parsed.scheme() != "https"
            && !(cfg!(debug_assertions) && parsed.scheme() == "file")
            || parsed.username() != ""
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err(PackageError::Invalid);
        }
        // Debug/test builds accept file:// catalog URLs for signed fixtures.
        if cfg!(debug_assertions) && parsed.scheme() == "file" {
            let path = parsed.to_file_path().map_err(|_| PackageError::Invalid)?;
            let body = fs::read(path).map_err(|_| PackageError::Invalid)?;
            if body.len() > MAX_ENVELOPE as usize {
                return Err(PackageError::Invalid);
            }
            let envelope: Envelope =
                serde_json::from_slice(&body).map_err(|_| PackageError::Invalid)?;
            let bytes = STANDARD
                .decode(envelope.bytes)
                .map_err(|_| PackageError::Invalid)?;
            return self.apply_catalog(bytes, envelope.signatures);
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
        self.install_from_path_locked(id, version, path.as_ref())
    }

    fn install_from_path_locked(
        &self,
        id: &str,
        version: &str,
        path: &Path,
    ) -> Result<PackageSummary, PackageError> {
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
        let _worker_guard = if let Some(worker) = self.worker.as_ref() {
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
        if worker_was_live {
            let worker = self.worker.as_ref().ok_or(PackageError::Persistence)?;
            if worker.supervisor.stop(id, version).await.is_err() {
                return Err(PackageError::Persistence);
            }
        }

        let (previous, should_restart, definition_snapshot, result, replacement) = {
            let _mutation = Self::lock(&self.mutation);
            let previous = self.store.installed(id, version).ok();
            let should_restart = worker_was_live
                && previous
                    .as_ref()
                    .is_some_and(|package| package.enabled && !package.revoked);
            let definition_snapshot = should_restart
                .then(|| self.package_registrations.snapshot())
                .transpose()
                .map_err(|_| PackageError::Persistence)?;
            let result = self.install_from_path_locked(id, version, path.as_ref());
            let replacement = self.store.installed(id, version).ok();
            (
                previous,
                should_restart,
                definition_snapshot,
                result,
                replacement,
            )
        };
        match result {
            Ok(summary) if !should_restart => Ok(summary),
            Ok(_) => {
                let Some(replacement) = replacement.as_ref() else {
                    return Err(PackageError::Persistence);
                };
                match self
                    .set_worker_enabled_locked(id, version, true, true, Some(replacement))
                    .await
                {
                    Ok(summary) => Ok(summary),
                    Err(error) => {
                        let _stop_result = if let Some(worker) = self.worker.as_ref() {
                            Some(worker.supervisor.stop(id, version).await)
                        } else {
                            None
                        };
                        let Some(previous) = previous.as_ref() else {
                            return Err(PackageError::Persistence);
                        };
                        let restored = {
                            let _mutation = Self::lock(&self.mutation);
                            if Some(replacement)
                                != self.store.installed(id, version).ok().as_ref()
                            {
                                return Err(PackageError::Persistence);
                            }
                            let restored = self
                                .restore_install_after_failure(
                                    definition_snapshot.as_deref(),
                                    Some(previous),
                                    id,
                                    version,
                                )
                                .is_ok()
                                && self.store.installed(id, version).ok().as_ref()
                                    == Some(previous);
                            if !restored {
                                if let Some(worker) = self.worker.as_ref() {
                                    worker.supervisor.revoke_typed_launch(id, version);
                                }
                                let _disable_failed = self.store.disable(id, version).is_err();
                            }
                            restored
                        };
                        if !restored {
                            return Err(PackageError::Persistence);
                        }
                        if self
                            .set_worker_enabled_locked(id, version, true, false, Some(previous))
                            .await
                            .is_err()
                        {
                            let _stop_failed = if let Some(worker) = self.worker.as_ref() {
                                worker.supervisor.stop(id, version).await.is_err()
                            } else {
                                false
                            };
                            let _disable_failed = self.disable_if_exact(previous).is_err();
                            return Err(PackageError::Persistence);
                        }
                        Err(error)
                    }
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
                    let _mutation = Self::lock(&self.mutation);
                    if replacement.as_ref()
                        != self.store.installed(id, version).ok().as_ref()
                    {
                        return Err(PackageError::Persistence);
                    }
                    if let Some(worker) = self.worker.as_ref() {
                        worker.supervisor.revoke_typed_launch(id, version);
                    }
                    let _disable_failed = self.store.disable(id, version).is_err();
                    return Err(PackageError::Persistence);
                }
                if self
                    .set_worker_enabled_locked(id, version, true, false, Some(previous))
                    .await
                    .is_err()
                {
                    let _stop_failed = if let Some(worker) = self.worker.as_ref() {
                        worker.supervisor.stop(id, version).await.is_err()
                    } else {
                        false
                    };
                    let _disable_failed = self.disable_if_exact(previous).is_err();
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
        if !installed.hash.eq_ignore_ascii_case(&entry.sha256) {
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

    async fn set_worker_enabled_locked(
        &self,
        id: &str,
        version: &str,
        enabled: bool,
        require_current_catalog: bool,
        expected: Option<&InstalledPackage>,
    ) -> Result<PackageSummary, PackageError> {
        let worker = self.worker.as_ref().ok_or(PackageError::Invalid)?;
        if !enabled {
            if expected.is_some_and(|expected| {
                let _mutation = Self::lock(&self.mutation);
                self.store.installed(id, version).ok().as_ref() != Some(expected)
            }) {
                return Err(PackageError::Persistence);
            }
            worker
                .supervisor
                .stop(id, version)
                .await
                .map_err(|_| PackageError::Persistence)?;
            if let Some(expected) = expected {
                if !self.disable_if_exact(expected)? {
                    return Err(PackageError::Persistence);
                }
            } else {
                self.disable(id, version)?;
            }
            return Ok(summary(
                self.store.installed(id, version)?,
                self.worker.as_ref(),
                &self.store,
            ));
        }
        let launch = self.prepare_worker_launch(id, version, require_current_catalog, expected)?;
        let previous = self
            .store
            .list()?
            .into_iter()
            .filter(|package| {
                package.id == id
                    && package.version != version
                    && package.enabled
                    && package.manifest.worker_entrypoint().is_some()
            })
            .map(|package| {
                let was_running = !matches!(
                    worker.supervisor.health(&package.id, &package.version).state,
                    WorkerState::Stopped
                );
                self.prepare_worker_launch(&package.id, &package.version, false, Some(&package))
                    .map(|launch| (launch, was_running))
            })
            .collect::<Result<Vec<_>, _>>()?;
        // A pre-existing double-enabled state cannot be rolled back safely.
        if previous.len() > 1 {
            return Err(PackageError::Persistence);
        }
        let mut stopped = Vec::new();
        for (old, was_running) in &previous {
            if *was_running {
                if worker.supervisor.stop(&old.expected.id, &old.expected.version).await.is_err() {
                    self.restore_stopped_workers(&stopped).await?;
                    return Err(PackageError::Persistence);
                }
                stopped.push(old.clone());
            }
        }
        if !previous.is_empty() {
            let versions = previous
                .iter()
                .map(|(old, _)| old.expected.version.clone())
                .collect::<Vec<_>>();
            if self.store.disable_worker_versions(id, &versions).is_err() {
                self.restore_stopped_workers(&stopped).await?;
                return Err(PackageError::Persistence);
            }
        }
        if let Err(error) = self.start_prepared_worker(&launch, require_current_catalog).await {
            let _ = worker.supervisor.stop(id, version).await;
            self.restore_worker_cutover(&launch, &previous).await?;
            return Err(error);
        }
        if self.store.enable_worker(id, version).is_err() {
            let _ = worker.supervisor.stop(id, version).await;
            worker.supervisor.revoke_typed_launch(id, version);
            self.restore_worker_cutover(&launch, &previous).await?;
            return Err(PackageError::Persistence);
        }
        Ok(summary(
            self.store.installed(id, version)?,
            self.worker.as_ref(),
            &self.store,
        ))
    }

    fn prepare_worker_launch(
        &self,
        id: &str,
        version: &str,
        require_current_catalog: bool,
        expected: Option<&InstalledPackage>,
    ) -> Result<PreparedWorkerLaunch, PackageError> {
        let _mutation = Self::lock(&self.mutation);
        let mut state = Self::lock(&self.state);
        let installed = self.store.installed(id, version)?;
        if expected.is_some_and(|expected| expected != &installed) {
            return Err(PackageError::Persistence);
        }
        if installed.revoked {
            return Err(PackageError::Invalid);
        }
        if self.store.list()?.into_iter().any(|other| {
            other.enabled
                && !other.revoked
                && other.id != installed.id
                && worker_scopes_overlap(&installed.manifest, &other.manifest)
        }) {
            return Err(PackageError::Invalid);
        }
        if require_current_catalog {
            let entry = Self::current_entry(&mut state, id, version)?;
            if !installed.hash.eq_ignore_ascii_case(&entry.sha256) {
                return Err(PackageError::Invalid);
            }
        }
        let executable = self.store.immutable_entrypoint(&installed)?;
        let state_root = self.root.join("package-state").join(id);
        ensure_owner_only_directory(&state_root).map_err(|_| PackageError::Persistence)?;
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
        let worker = self.worker.as_ref().ok_or(PackageError::Invalid)?;
        let roots = if matches!(installed.manifest.kind(), PackageKind::Bridge) {
            self.bridge_roots(id, version)?
        } else {
            worker.roots.clone()
        };
        let integration = self.integration_launch_config(&installed)?;
        let typed_grant = self.ensure_typed_grant(&installed)?;
        let mut manifest = installed.manifest.common_manifest();
        manifest.permissions = worker_permissions(&installed.manifest);
        if matches!(manifest.kind, PackageKind::App) {
            manifest.kind = PackageKind::Source;
        }
        manifest.entrypoint = installed
            .manifest
            .worker_entrypoint()
            .ok_or(PackageError::Invalid)?
            .to_owned();
        Ok(PreparedWorkerLaunch {
            manifest,
            hash: installed.hash.clone(),
            executable,
            state_root,
            roots,
            bridge_config,
            integration,
            typed_grant,
            expected: installed,
        })
    }

    async fn start_prepared_worker(
        &self,
        launch: &PreparedWorkerLaunch,
        require_current_catalog: bool,
    ) -> Result<(), PackageError> {
        let worker = self.worker.as_ref().ok_or(PackageError::Invalid)?;
        let id = &launch.expected.id;
        let version = &launch.expected.version;
        worker
            .supervisor
            .bind_typed_launch(id, version, &worker.correlation_id, 1, launch.typed_grant.clone())
            .map_err(|_| {
                worker.supervisor.revoke_typed_launch(id, version);
                PackageError::Worker("unavailable")
            })?;
        if let Err(error) = worker
            .supervisor
            .start(
                &launch.manifest,
                launch.executable.clone(),
                launch.state_root.clone(),
                launch.hash.clone(),
                &launch.roots,
                worker.correlation_id.clone(),
                launch.bridge_config.clone(),
                launch.integration.clone(),
            )
            .await
        {
            worker.supervisor.revoke_typed_launch(id, version);
            return Err(PackageError::Worker(match error {
                "worker-required" | "unsupported-platform" | "already-running" => {
                    "invalid-request"
                }
                _ => "unavailable",
            }));
        }
        let catalog_matches = if require_current_catalog {
            let mut state = Self::lock(&self.state);
            Self::current_entry(&mut state, id, version)
                .is_ok_and(|entry| launch.expected.hash.eq_ignore_ascii_case(&entry.sha256))
        } else {
            true
        };
        let exact_record = self.store.installed(id, version).ok().as_ref() == Some(&launch.expected);
        if !exact_record || !catalog_matches || !worker.supervisor.activate(id, version) {
            let _ = worker.supervisor.stop(id, version).await;
            worker.supervisor.revoke_typed_launch(id, version);
            return Err(PackageError::Persistence);
        }
        Ok(())
    }

    async fn restore_stopped_workers(
        &self,
        workers: &[PreparedWorkerLaunch],
    ) -> Result<(), PackageError> {
        for worker in workers {
            self.start_prepared_worker(worker, false).await?;
            self.store.enable_worker(&worker.expected.id, &worker.expected.version)?;
        }
        Ok(())
    }

    async fn restore_worker_cutover(
        &self,
        selected: &PreparedWorkerLaunch,
        previous: &[(PreparedWorkerLaunch, bool)],
    ) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        self.store.disable(&selected.expected.id, &selected.expected.version)?;
        if let Some((old, _)) = previous.first() {
            self.store.enable_worker(&old.expected.id, &old.expected.version)?;
        }
        drop(_mutation);
        self.restore_stopped_workers(
            &previous
                .iter()
                .filter(|(_, was_running)| *was_running)
                .map(|(old, _)| old.clone())
                .collect::<Vec<_>>(),
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

#[derive(Debug, Clone)]
struct PreparedWorkerLaunch {
    manifest: PackageManifest,
    hash: String,
    executable: PathBuf,
    state_root: PathBuf,
    roots: Vec<PathBuf>,
    bridge_config: Option<BridgeWorkerConfig>,
    integration: Option<IntegrationLaunchConfig>,
    typed_grant: LaunchGrant,
    expected: InstalledPackage,
}

fn worker_scopes_overlap(left: &VersionedManifest, right: &VersionedManifest) -> bool {
    let scopes = |manifest: &VersionedManifest| {
        manifest
            .permissions()
            .iter()
            .filter(|permission| permission.capability == "worker.invoke")
            .flat_map(|permission| permission.scopes.iter().cloned())
            .collect::<Vec<_>>()
    };
    let left = scopes(left);
    let right = scopes(right);
    left.iter().any(|left| {
        right.iter().any(|right| {
            scope_contains(left, right) || scope_contains(right, left)
        })
    })
}

fn worker_permissions(manifest: &VersionedManifest) -> Vec<PermissionRequest> {
    let v2_worker = matches!(manifest, VersionedManifest::V2(_));
    manifest
        .permissions()
        .iter()
        .filter_map(|permission| {
            if v2_worker && matches!(permission.capability.as_str(), "ark.read" | "ark.write") {
                // ponytail: v2 UI ARK scopes stay in the Host principal; the worker gets only
                // its explicit dictation.control and worker.invoke authority.
                let scopes = permission
                    .scopes
                    .iter()
                    .filter(|scope| !scope.starts_with("dictation."))
                    .cloned()
                    .collect::<Vec<_>>();
                (!scopes.is_empty()).then(|| PermissionRequest {
                    capability: permission.capability.clone(),
                    scopes,
                })
            } else {
                Some(permission.clone())
            }
        })
        .collect()
}

fn scope_contains(scope: &str, operation: &str) -> bool {
    scope == operation
        || scope.strip_suffix(".*").is_some_and(|prefix| {
            operation == prefix || operation.starts_with(&format!("{prefix}."))
        })
}

#[cfg(test)]
mod worker_scope_tests {
    use super::{scope_contains, worker_permissions};
    use crate::package_manifest::PackageManifest;

    #[test]
    fn wildcard_scope_owns_only_its_namespace() {
        assert!(scope_contains("games.*", "games.list"));
        assert!(scope_contains("games.*", "games"));
        assert!(!scope_contains("games.*", "games2.list"));
    }

    #[test]
    fn v2_worker_projection_keeps_worker_grants_out_of_ui_ark_scopes() {
        let raw = r#"{
            "schema_version": 2, "id": "com.kosmos.dictation", "name": "Dictation",
            "version": "0.2.5", "kind": "app", "engine_api": ">=1.0.0",
            "entrypoint": "dist/index.html", "publisher": "kosmos",
            "permissions": [
                {"capability": "dictation.control", "scopes": ["dictation.capture.start"]},
                {"capability": "worker.invoke", "scopes": ["dictation.trigger"]},
                {"capability": "ark.read", "scopes": ["dictation.get_state"]},
                {"capability": "ark.write", "scopes": ["dictation.cancel"]}
            ],
            "targets": [{"runtime": "worker", "os": ["windows"], "entrypoint": "worker.exe"}],
            "data": {"access": [], "defines": [], "mappings": []}
        }"#;
        let manifest = PackageManifest::parse(raw).expect("valid v2 worker manifest");
        let projected = worker_permissions(&manifest);
        assert_eq!(projected.len(), 2);
        assert!(projected.iter().any(|p| p.capability == "dictation.control"));
        assert!(projected.iter().any(|p| p.capability == "worker.invoke"));
        assert!(!projected.iter().any(|p| p.scopes.iter().any(|s| s == "dictation.get_state")));
    }
}
