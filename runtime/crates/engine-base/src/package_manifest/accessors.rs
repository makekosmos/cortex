use super::{
    ManifestError, ManifestStore, ManifestTarget, PackageKind, PackageManifest, PermissionRequest,
    TargetOs, TargetRuntime, VersionedManifest,
};

impl VersionedManifest {
    /// Common package metadata used by the legacy store/launch surfaces.
    /// Version-specific fields stay in the enum variant and are persisted by
    /// callers that need the full v2 data contract.
    pub fn common_manifest(&self) -> PackageManifest {
        match self {
            Self::V1(manifest) => manifest.clone(),
            Self::V2(manifest) => PackageManifest {
                schema_version: 1,
                id: manifest.id.clone(),
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                kind: manifest.kind.clone(),
                engine_api: manifest.engine_api.clone(),
                entrypoint: manifest.entrypoint.clone(),
                publisher: manifest.publisher.clone(),
                permissions: manifest.permissions.clone(),
            },
        }
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        match self {
            Self::V1(manifest) => manifest.validate(),
            Self::V2(manifest) => manifest.validate(),
        }
    }

    pub fn schema_version(&self) -> u32 {
        match self {
            Self::V1(manifest) => manifest.schema_version,
            Self::V2(manifest) => manifest.schema_version,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.id,
            Self::V2(manifest) => &manifest.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.name,
            Self::V2(manifest) => &manifest.name,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.version,
            Self::V2(manifest) => &manifest.version,
        }
    }

    pub fn kind(&self) -> &PackageKind {
        match self {
            Self::V1(manifest) => &manifest.kind,
            Self::V2(manifest) => &manifest.kind,
        }
    }

    pub fn engine_api(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.engine_api,
            Self::V2(manifest) => &manifest.engine_api,
        }
    }

    pub fn entrypoint(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.entrypoint,
            Self::V2(manifest) => &manifest.entrypoint,
        }
    }

    pub fn icon(&self) -> Option<&str> {
        match self {
            Self::V1(_) => None,
            Self::V2(manifest) => manifest.icon.as_deref(),
        }
    }

    pub fn worker_entrypoint(&self) -> Option<&str> {
        match self {
            Self::V1(manifest)
                if matches!(manifest.kind, PackageKind::Source | PackageKind::Bridge) =>
            {
                Some(&manifest.entrypoint)
            }
            Self::V2(manifest) => manifest
                .targets
                .iter()
                .filter(|target| target.runtime == TargetRuntime::Worker)
                .find(|target| target.supports_current())
                .and_then(|target| target.entrypoint.as_deref())
                .or_else(|| {
                    (matches!(manifest.kind, PackageKind::Source | PackageKind::Bridge)
                        && manifest.targets.iter().any(|target| {
                            target.runtime == TargetRuntime::Worker && target.supports_current()
                        }))
                    .then_some(manifest.entrypoint.as_str())
                }),
            _ => None,
        }
    }

    pub fn publisher(&self) -> &str {
        match self {
            Self::V1(manifest) => &manifest.publisher,
            Self::V2(manifest) => &manifest.publisher,
        }
    }

    pub fn store(&self) -> Option<&ManifestStore> {
        match self {
            Self::V1(_) => None,
            Self::V2(manifest) => manifest.store.as_ref(),
        }
    }

    /// Store-facing description: the `store` block wins over the plain
    /// manifest description.
    pub fn store_description(&self) -> Option<&str> {
        match self {
            Self::V1(_) => None,
            Self::V2(manifest) => manifest
                .store
                .as_ref()
                .and_then(|store| store.description.as_deref())
                .or(manifest.description.as_deref()),
        }
    }

    /// Declared platform targets; a v1 manifest predates the target list.
    pub fn targets(&self) -> &[ManifestTarget] {
        match self {
            Self::V1(_) => &[],
            Self::V2(manifest) => &manifest.targets,
        }
    }

    /// Unique target operating systems in manifest order — the Store listing
    /// `availability.platforms` vocabulary is derived from these.
    pub fn target_platforms(&self) -> Vec<TargetOs> {
        match self {
            Self::V1(_) => vec![],
            Self::V2(manifest) => {
                let mut seen = std::collections::BTreeSet::new();
                let mut platforms = Vec::new();
                for target in &manifest.targets {
                    for os in &target.os {
                        if seen.insert(os.clone()) {
                            platforms.push(os.clone());
                        }
                    }
                }
                platforms
            }
        }
    }

    pub fn permissions(&self) -> &[PermissionRequest] {
        match self {
            Self::V1(manifest) => &manifest.permissions,
            Self::V2(manifest) => &manifest.permissions,
        }
    }
}
