impl PackageService {
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
}
