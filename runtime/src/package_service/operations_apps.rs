impl PackageService {
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
        let catalog_match = entry
            .archive()
            .is_some_and(|archive| archive.sha256.eq_ignore_ascii_case(&package.hash));
        if entry.manifest != package.manifest || !catalog_match {
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
}
