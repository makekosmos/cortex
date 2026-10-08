impl PackageService {
    /// Whether the catalog currently lists this exact package release.
    pub fn has_catalog_release(
        &self,
        id: &str,
        version: &str,
        expected_kind: &crate::package_manifest::PackageKind,
    ) -> bool {
        let state = Self::lock(&self.state);
        let Some(catalog) = state.catalog.as_ref() else {
            return false;
        };
        let Some(entry) = catalog.document.entry(id, version) else {
            return false;
        };
        entry.manifest.kind() == expected_kind
            && entry.archive().is_some()
            && !catalog.document.entry_revoked(entry)
    }

    pub fn catalog_packages(
        &self,
        kind: Option<&PackageKind>,
    ) -> Result<Vec<CatalogPackageSummary>, PackageError> {
        let state = Self::lock(&self.state);
        let catalog = state
            .catalog
            .as_ref()
            .ok_or(PackageError::CatalogUnavailable)?;
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
                archive_size: entry.archive().map_or(0, |archive| archive.size),
                revoked: catalog.document.entry_revoked(entry),
            })
            .collect())
    }

    /// Fetch the catalog from its one published location. `MUNDUS_PACKAGE_CATALOG_URL`
    /// overrides the URL for tests and diagnostics; debug/test builds may
    /// point it at a `file://` fixture — release builds require HTTPS.
    pub async fn refresh_catalog(&self) -> Result<CatalogSummary, PackageError> {
        let url = std::env::var("MUNDUS_PACKAGE_CATALOG_URL")
            .unwrap_or_else(|_| crate::catalog::PRODUCTION_CATALOG_URL.to_string());
        let parsed = reqwest::Url::parse(&url).map_err(|_| PackageError::Invalid)?;
        if parsed.scheme() != "https" && !(cfg!(debug_assertions) && parsed.scheme() == "file")
            || parsed.username() != ""
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err(PackageError::Invalid);
        }
        let body = if cfg!(debug_assertions) && parsed.scheme() == "file" {
            let path = parsed.to_file_path().map_err(|_| PackageError::Invalid)?;
            fs::read(path).map_err(|_| PackageError::Invalid)?
        } else {
            let response = engine_base::http::client_builder()
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
            read_body_bounded(response, crate::catalog::MAX_DOCUMENT as u64).await?
        };
        self.apply_catalog(&body)
    }

    /// Apply one catalog document: validate, persist, then reconcile
    /// revocations against the installed set. A catalog identical to the
    /// current one is an idempotent success — refresh is not a replay attack.
    pub fn apply_catalog(&self, bytes: impl AsRef<[u8]>) -> Result<CatalogSummary, PackageError> {
        let _mutation = Self::lock(&self.mutation);
        let bytes = bytes.as_ref();
        if bytes.len() > crate::catalog::MAX_DOCUMENT {
            return Err(PackageError::Invalid);
        }
        let mut state = Self::lock(&self.state);
        let min_sequence = state
            .catalog
            .as_ref()
            .map_or(0, |catalog| catalog.document.sequence);
        let document = match CatalogDocument::parse(bytes, Utc::now(), min_sequence) {
            Ok(document) => document,
            Err(CatalogError::Replay)
                if serde_json::from_slice::<CatalogDocument>(bytes)
                    .ok()
                    .as_ref()
                    == state.catalog.as_ref().map(|catalog| &catalog.document) =>
            {
                return Ok(catalog_summary(state.catalog.as_ref().expect("catalog")));
            }
            Err(error) => return Err(error.into()),
        };
        if self.persist_catalog(&document).is_err() {
            Self::disable_catalog(&mut state);
            return Err(PackageError::Persistence);
        }
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
            return Err(PackageError::Persistence);
        }
        let summary = CatalogSummary {
            sequence: document.sequence,
            expires_at: document.expires_at.clone(),
            package_count: document.packages.len(),
            revoked_count: document.revoked.len(),
        };
        state.catalog = Some(CatalogState { document });
        state.fault = None;
        Ok(summary)
    }

    /// Storefront projection for Manager — listings derived from the one
    /// document plus the installed state the caller supplies.
    pub fn store_catalog(
        &self,
        now: DateTime<Utc>,
        installed: Vec<InstalledListing>,
    ) -> crate::catalog::CatalogDto {
        let state = Self::lock(&self.state);
        match state.catalog.as_ref() {
            Some(catalog) => {
                let fresh = catalog
                    .document
                    .expires_at()
                    .is_some_and(|expiry| expiry > now);
                crate::catalog::CatalogDto {
                    state: if fresh {
                        "fresh".into()
                    } else {
                        "expired".into()
                    },
                    platform: crate::catalog::Platform::current(),
                    sequence: Some(catalog.document.sequence),
                    issued_at: Some(catalog.document.issued_at.clone()),
                    expires_at: Some(catalog.document.expires_at.clone()),
                    listings: catalog.document.listings(),
                    installed,
                }
            }
            None => crate::catalog::CatalogDto {
                state: "unavailable".into(),
                platform: crate::catalog::Platform::current(),
                sequence: None,
                issued_at: None,
                expires_at: None,
                listings: Vec::new(),
                installed,
            },
        }
    }

    /// Resolve a Store-provided external-app URL. Stale and unavailable
    /// catalogs are intentionally unusable.
    pub fn store_external_url(&self, listing_id: &str) -> Result<String, PackageError> {
        if listing_id.is_empty()
            || listing_id.len() > 128
            || listing_id.chars().any(char::is_control)
        {
            return Err(PackageError::Invalid);
        }
        let state = Self::lock(&self.state);
        let catalog = state
            .catalog
            .as_ref()
            .ok_or(PackageError::CatalogUnavailable)?;
        if catalog
            .document
            .expires_at()
            .is_none_or(|expiry| expiry <= Utc::now())
        {
            return Err(PackageError::CatalogUnavailable);
        }
        catalog
            .document
            .external_url(listing_id)
            .ok_or(PackageError::Invalid)
    }
}

fn catalog_summary(catalog: &CatalogState) -> CatalogSummary {
    CatalogSummary {
        sequence: catalog.document.sequence,
        expires_at: catalog.document.expires_at.clone(),
        package_count: catalog.document.packages.len(),
        revoked_count: catalog.document.revoked.len(),
    }
}
