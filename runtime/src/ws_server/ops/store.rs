use super::*;
use crate::package_manifest::PackageKind;
use crate::store_catalog::{PackageReleaseKind, Platform};
struct StoreCatalogIndex<'a> {
    packages: &'a PackageService,
}

impl PackageIndexLookup for StoreCatalogIndex<'_> {
    fn package_release(&self, package_id: &str, version: &str, kind: PackageReleaseKind) -> bool {
        self.packages.has_catalog_release(
            package_id,
            version,
            match kind {
                PackageReleaseKind::App => &PackageKind::App,
                PackageReleaseKind::Source => &PackageKind::Source,
                PackageReleaseKind::Bridge => &PackageKind::Bridge,
            },
        )
    }

    fn canonical_type_version(&self, type_id: &str, versions: &str) -> bool {
        let Ok(requirement) = semver::VersionReq::parse(versions) else {
            return false;
        };
        ark_core::canonical_types::definitions::canonical_type_registrations()
            .ok()
            .is_some_and(|types| {
                types.into_iter().any(|registered| {
                    registered.type_id == type_id
                        && semver::Version::parse(&registered.version)
                            .ok()
                            .is_some_and(|version| requirement.matches(&version))
                })
            })
    }
}

pub(in crate::ws_server) async fn handle_store_op(
    subop: &str,
    params: serde_json::Value,
    packages: &PackageService,
    catalog: Option<&StoreCatalogService>,
) -> LocalResponse {
    if subop == "external_url" {
        let listing_id = params.get("listing_id").and_then(Value::as_str);
        return match (catalog, listing_id) {
            (Some(catalog), Some(listing_id)) => catalog
                .external_url_at(listing_id, chrono::Utc::now())
                .map(|url| LocalResponse::ok(serde_json::json!({ "url": url })))
                .unwrap_or_else(|_| LocalResponse::err("store: unavailable")),
            _ => LocalResponse::err("store: unavailable"),
        };
    }
    let installed = match packages.store_installed_listings() {
        Ok(installed) => installed,
        Err(_) => return LocalResponse::err("store: installed-packages-unavailable"),
    };
    let response = match subop {
        "catalog" => Ok(match catalog {
            Some(catalog) => catalog.catalog(chrono::Utc::now(), installed),
            None => CatalogDto {
                state: "unavailable".into(),
                platform: Platform::current(),
                sequence: None,
                issued_at: None,
                expires_at: None,
                listings: Vec::new(),
                installed,
            },
        }),
        "refresh" => match catalog {
            Some(catalog) => {
                if packages.catalog_summary().is_none() && packages.refresh_catalog().await.is_err()
                {
                    Err("store: package-index-unavailable")
                } else {
                    match catalog.refresh(&StoreCatalogIndex { packages }).await {
                        Ok(_) => Ok(catalog.catalog(
                            chrono::Utc::now(),
                            packages.store_installed_listings().unwrap_or_default(),
                        )),
                        Err(_) => Err("store: refresh-unavailable"),
                    }
                }
            }
            None => Err("store: unavailable"),
        },
        other => return LocalResponse::err(format!("store.{other}: unknown sub-operation")),
    };
    match response
        .and_then(|result| serde_json::to_value(result).map_err(|_| "store: serialization-failed"))
    {
        Ok(value) => LocalResponse::ok(value),
        Err(error) => LocalResponse::err(error),
    }
}
