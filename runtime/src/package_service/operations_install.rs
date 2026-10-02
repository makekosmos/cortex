impl PackageService {
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
        let package =
            self.store
                .install_versioned(path, bytes.len() as u64, &hash, &manifest, 0)?;
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
                            if Some(replacement) != self.store.installed(id, version).ok().as_ref()
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
            }
            Err(error) => {
                if !should_restart {
                    return Err(error);
                }
                let Some(previous) = previous.as_ref() else {
                    return Err(PackageError::Persistence);
                };
                if !restored_package_record_matches(&self.store, previous, id, version) {
                    let _mutation = Self::lock(&self.mutation);
                    if replacement.as_ref() != self.store.installed(id, version).ok().as_ref() {
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
}
