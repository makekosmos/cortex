impl PackageService {
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
        let metadata = retry_io(|| fs::metadata(path)).map_err(|_| PackageError::Persistence)?;
        if !metadata.is_file() || metadata.len() > MAX_ENVELOPE {
            return Err(PackageError::Invalid);
        }
        let env: Envelope = serde_json::from_slice(
            &retry_io(|| fs::read(path)).map_err(|_| PackageError::Persistence)?,
        )
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
