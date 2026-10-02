impl PackageService {
    /// Test-only constructor for signed fixture catalogs. Production builds
    /// cannot select writable trust roots; the fixture feature is compiled
    /// only for the local worker acceptance harness.
    #[cfg(feature = "package-worker-fixture")]
    pub fn open_with_test_trust(
        data_dir: impl AsRef<Path>,
        root_key: TrustedKey,
        release_keys: Vec<TrustedKey>,
    ) -> Result<Self, PackageError> {
        let trust = TrustStore::new(root_key, release_keys).map_err(PackageError::Trust)?;
        // Test/fixture constructors scope every product root — including the
        // Apps dir — to the passed data_dir so nothing touches real user dirs.
        Self::from_parts(
            data_dir.as_ref().join("packages"),
            Some(trust),
            Some(data_dir.as_ref().join("apps")),
        )
    }

    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, PackageError> {
        let root = data_dir.as_ref().join("packages");
        retry_io(|| fs::create_dir_all(&root)).map_err(|_| PackageError::Persistence)?;
        let trust = match (
            option_env!("MUNDUS_PACKAGE_ROOT_KEY_JSON"),
            option_env!("MUNDUS_PACKAGE_RELEASE_KEYS_JSON"),
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
        // Apps root — single source: `native_apps::native_apps_root()` (the
        // product-local Mundus dir, honoring the env override).
        Self::from_parts(root, trust, crate::native_apps::native_apps_root().ok())
    }
    #[cfg(test)]
    pub fn open_with_trust(
        data_dir: impl AsRef<Path>,
        trust: TrustStore,
    ) -> Result<Self, PackageError> {
        Self::from_parts(
            data_dir.as_ref().join("packages"),
            Some(trust),
            Some(data_dir.as_ref().join("apps")),
        )
    }

    /// Crate-internal constructor — the dispatcher tests and the native
    /// tests build minimal services without a trust store.
    pub(crate) fn from_parts(
        root: PathBuf,
        trust: Option<TrustStore>,
        apps_root: Option<PathBuf>,
    ) -> Result<Self, PackageError> {
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
        let native_apps = apps_root
            .and_then(|root| crate::native_apps::NativeAppStore::new(root).ok())
            .map(std::sync::Arc::new);
        let service = Self {
            store,
            root,
            native_apps,
            release_cache: Mutex::new(HashMap::new()),
            release_failures: Mutex::new(HashMap::new()),
            native_jobs: std::sync::Arc::new(Mutex::new(HashMap::new())),
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
            background_tasks: Mutex::new(Vec::new()),
            launch_surface: Mutex::new(None),
            typed_registry: Mutex::new(typed_registry),
            grants: std::sync::Arc::new(GrantAuthorityRegistry::with_data_dir(data_dir)),
        };
        // A stale installed v2 contract must not brick Engine startup. Invalid
        // packages stay installed but disabled and cannot be enabled/launched.
        let _ = service.rebuild_typed_grants();
        service.replay();
        Ok(service)
    }

    /// Register a spawned task that holds `Arc<PackageService>` — the
    /// registry prunes completed handles on push, so it stays bounded by the
    /// number of live tasks. Used by the service's own install jobs and by
    /// the Engine for its one-shot startup tasks so `drain_background` can
    /// join them at shutdown.
    pub fn track_background_task(
        &self,
        label: impl Into<String>,
        task: tokio::task::JoinHandle<()>,
    ) {
        crate::background_task::track_task(&self.background_tasks, label, task);
    }

    /// Join every tracked background task. Called at Engine shutdown so no
    /// install/migration write into the data dir outlives the service.
    pub async fn drain_background(&self) {
        crate::background_task::drain_tasks(&self.background_tasks, BACKGROUND_JOIN_TIMEOUT).await;
    }
}
