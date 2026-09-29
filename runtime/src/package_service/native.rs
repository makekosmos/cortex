// Native app operations (`apps.*` RPCs). Native apps are `kind: "app"`
// catalog entries carrying a `native` descriptor; everything install-facing
// (freshness, trust, revocation, engine_api compatibility) flows through the
// same signed-catalog checks as `.kspkg` packages via `current_entry`.

use crate::native_apps::{NativeAppInstall, NativeInstallSpec};
use crate::package_trust::{host_native_target, NativeArtifact};

/// Manager-facing row for one native app.
#[derive(Debug, Clone, Serialize)]
pub struct NativeAppSummary {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub installed: bool,
    pub installed_version: Option<String>,
    /// Latest version the signed catalog offers for this host's target.
    pub catalog_version: Option<String>,
    /// Set when the catalog offers a newer version than the install.
    pub update_version: Option<String>,
    pub revoked: bool,
}

impl PackageService {
    fn native_store(&self) -> Result<&crate::native_apps::NativeAppStore, PackageError> {
        self.native_apps.as_ref().ok_or(PackageError::Invalid)
    }

    /// Resolve the catalog entry for a native app; `None` picks the latest
    /// native version published for this id. Runs the full
    /// freshness/trust/revocation gate of `current_entry`.
    fn native_entry(
        state: &mut State,
        id: &str,
        version: Option<&str>,
    ) -> Result<(CatalogEntry, NativeArtifact), PackageError> {
        let version = match version {
            Some(version) => version.to_owned(),
            None => {
                let catalog = state.catalog.as_ref().ok_or(PackageError::TrustUnavailable)?;
                catalog
                    .document
                    .packages
                    .iter()
                    .filter(|entry| {
                        entry.manifest.id() == id && entry.native.is_some()
                    })
                    .filter_map(|entry| {
                        semver::Version::parse(entry.manifest.version())
                            .ok()
                            .map(|v| (v, entry.manifest.version()))
                    })
                    .max_by(|a, b| a.0.cmp(&b.0))
                    .map(|(_, v)| v.to_owned())
                    .ok_or(PackageError::Invalid)?
            }
        };
        let entry = Self::current_entry(state, id, &version)?;
        let native = entry.native.clone().ok_or(PackageError::Invalid)?;
        Ok((entry, native))
    }

    /// Latest native version the catalog offers this id, if any.
    fn native_catalog_version(state: &State, id: &str) -> Option<String> {
        state
            .catalog
            .as_ref()?
            .document
            .packages
            .iter()
            .filter(|entry| entry.manifest.id() == id && entry.native.is_some())
            .filter_map(|entry| semver::Version::parse(entry.manifest.version()).ok())
            .max()
            .map(|v| v.to_string())
    }

    fn native_display(state: &State, id: &str) -> (String, Option<String>) {
        let Some(catalog) = state.catalog.as_ref() else {
            return (id.to_owned(), None);
        };
        let Some(entry) = catalog
            .document
            .packages
            .iter()
            .filter(|entry| entry.manifest.id() == id && entry.native.is_some())
            .max_by_key(|entry| entry.manifest.version())
        else {
            return (id.to_owned(), None);
        };
        (
            entry.manifest.name().to_owned(),
            entry.manifest.icon().map(str::to_owned),
        )
    }

    fn native_summary(
        &self,
        state: &State,
        record: Option<&NativeAppInstall>,
        id: &str,
    ) -> NativeAppSummary {
        let (name, icon) = Self::native_display(state, id);
        let catalog_version = Self::native_catalog_version(state, id);
        let installed_version = record.map(|record| record.version.clone());
        let update_version = match (&installed_version, &catalog_version) {
            (Some(installed), Some(catalog)) => {
                let installed_semver = semver::Version::parse(installed).ok();
                let catalog_semver = semver::Version::parse(catalog).ok();
                match (installed_semver, catalog_semver) {
                    (Some(installed), Some(catalog)) if catalog > installed => Some(catalog.to_string()),
                    _ => None,
                }
            }
            _ => None,
        };
        let revoked = record.is_some_and(|record| {
            state.trust.as_ref().is_some_and(|trust| {
                trust.is_package_revoked(&record.id, &record.version, &record.sha256)
            })
        });
        NativeAppSummary {
            id: id.to_owned(),
            name,
            icon,
            installed: record.is_some(),
            installed_version,
            catalog_version,
            update_version,
            revoked,
        }
    }

    /// Union of installed native apps and catalog-published native entries.
    pub fn native_apps(&self) -> Result<Vec<NativeAppSummary>, PackageError> {
        let store = self.native_store()?;
        let state = Self::lock(&self.state);
        let mut ids: std::collections::BTreeSet<String> = store
            .list()
            .into_iter()
            .map(|record| record.id)
            .collect();
        if let Some(catalog) = state.catalog.as_ref() {
            for entry in &catalog.document.packages {
                if entry.native.is_some() {
                    ids.insert(entry.manifest.id().to_owned());
                }
            }
        }
        Ok(ids
            .into_iter()
            .map(|id| {
                let record = store.current(&id);
                self.native_summary(&state, record.as_ref(), &id)
            })
            .collect())
    }
}


fn native_store_error(error: crate::native_apps::NativeAppError) -> PackageError {
    match error {
        crate::native_apps::NativeAppError::Running => PackageError::Worker("app-running"),
        crate::native_apps::NativeAppError::HashMismatch
        | crate::native_apps::NativeAppError::SizeMismatch => PackageError::Invalid,
        crate::native_apps::NativeAppError::Invalid(_) => PackageError::Invalid,
        _ => PackageError::Persistence,
    }
}
