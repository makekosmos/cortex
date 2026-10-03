impl PackageService {
    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn disable_catalog(state: &mut State) {
        state.catalog = None;
        state.fault = Some("catalog_unavailable".into());
    }

    /// Startup catalog load plus the KOS-320 migration: the signed-catalog
    /// layout (envelope `catalog.json`, `transition-*.json`,
    /// `revocation-*.json`, and the store dir's signed envelope) is dropped
    /// on sight — none of it can produce a valid unsigned document, so the
    /// service simply starts catalog-free and the next refresh repopulates.
    /// Installed packages (`state.json`, blobs, unpacked trees) are never
    /// touched by this cleanup.
    fn replay(&self) {
        let _mutation = Self::lock(&self.mutation);
        self.drop_legacy_trust_files();
        let mut state = Self::lock(&self.state);
        let path = self.root.join("catalog.json");
        if path.exists() {
            match self.load_catalog(&path) {
                Ok(document) => {
                    if self
                        .store
                        .reconcile_revocations(document.revoked.iter().map(|item| {
                            (
                                item.id.as_str(),
                                item.version.as_str(),
                                item.sha256.as_str(),
                            )
                        }))
                        .is_err()
                    {
                        Self::disable_catalog(&mut state);
                        return;
                    }
                    state.catalog = Some(CatalogState { document });
                    state.fault = None;
                }
                Err(_) => Self::disable_catalog(&mut state),
            }
        }
    }

    fn drop_legacy_trust_files(&self) {
        for name in ["transition-", "revocation-"] {
            for path in self.document_paths(name) {
                let _ = fs::remove_file(path);
            }
        }
        // The signed store catalog persisted beside the package root.
        if let Some(data_dir) = self.root.parent() {
            let legacy = data_dir.join("store").join("catalog.json");
            if legacy.exists() {
                let _ = fs::remove_file(legacy);
            }
        }
        // An old signed envelope can never parse as the new document; remove
        // it up front so a partially migrated dir does not confuse startup.
        let catalog_path = self.root.join("catalog.json");
        if catalog_path.exists() {
            let keep = fs::read(&catalog_path)
                .ok()
                .and_then(|bytes| CatalogDocument::parse_persisted(&bytes).ok())
                .is_some();
            if !keep {
                let _ = fs::remove_file(&catalog_path);
            }
        }
    }

    fn load_catalog(&self, path: &Path) -> Result<CatalogDocument, PackageError> {
        let bytes = retry_io(|| fs::read(path)).map_err(|_| PackageError::Persistence)?;
        CatalogDocument::parse_persisted(&bytes).map_err(PackageError::Catalog)
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

    fn persist_catalog(&self, document: &CatalogDocument) -> Result<(), PackageError> {
        write_owner_only_json(&self.root.join("catalog.json"), document)
            .map_err(|_| PackageError::Persistence)
    }

    fn current_entry(
        state: &mut State,
        id: &str,
        version: &str,
    ) -> Result<CatalogEntry, PackageError> {
        let Some(catalog) = state.catalog.as_ref() else {
            return Err(PackageError::CatalogUnavailable);
        };
        let expiry = catalog.document.expires_at().ok_or(PackageError::Invalid)?;
        if expiry <= Utc::now() {
            Self::disable_catalog(state);
            return Err(PackageError::CatalogUnavailable);
        }
        let entry = catalog
            .document
            .entry(id, version)
            .cloned()
            .ok_or(PackageError::Invalid)?;
        if catalog
            .document
            .is_revoked(entry.manifest.id(), entry.manifest.version(), &entry.sha256)
        {
            return Err(PackageError::Invalid);
        }
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
