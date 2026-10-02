impl PackageService {
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
                    worker
                        .supervisor
                        .health(&package.id, &package.version)
                        .state,
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
                if worker
                    .supervisor
                    .stop(&old.expected.id, &old.expected.version)
                    .await
                    .is_err()
                {
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
        if let Err(error) = self
            .start_prepared_worker(&launch, require_current_catalog)
            .await
        {
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
            .bind_typed_launch(
                id,
                version,
                &worker.correlation_id,
                1,
                launch.typed_grant.clone(),
            )
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
                "worker-required" | "unsupported-platform" | "already-running" => "invalid-request",
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
        let exact_record =
            self.store.installed(id, version).ok().as_ref() == Some(&launch.expected);
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
            self.store
                .enable_worker(&worker.expected.id, &worker.expected.version)?;
        }
        Ok(())
    }

    async fn restore_worker_cutover(
        &self,
        selected: &PreparedWorkerLaunch,
        previous: &[(PreparedWorkerLaunch, bool)],
    ) -> Result<(), PackageError> {
        let _mutation = Self::lock(&self.mutation);
        self.store
            .disable(&selected.expected.id, &selected.expected.version)?;
        if let Some((old, _)) = previous.first() {
            self.store
                .enable_worker(&old.expected.id, &old.expected.version)?;
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
        right
            .iter()
            .any(|right| scope_contains(left, right) || scope_contains(right, left))
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
        assert!(projected
            .iter()
            .any(|p| p.capability == "dictation.control"));
        assert!(projected.iter().any(|p| p.capability == "worker.invoke"));
        assert!(!projected
            .iter()
            .any(|p| p.scopes.iter().any(|s| s == "dictation.get_state")));
    }
}
