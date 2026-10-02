impl PackageService {
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
        entry.manifest.kind() == expected_kind && trust.ensure_package_allowed(entry).is_ok()
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
        if parsed.scheme() != "https" && !(cfg!(debug_assertions) && parsed.scheme() == "file")
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
}
