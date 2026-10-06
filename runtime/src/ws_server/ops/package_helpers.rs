use super::*;

pub(in crate::ws_server) fn package_id_version<'a>(
    subop: &str,
    params: &'a serde_json::Value,
) -> Result<(&'a str, &'a str), LocalResponse> {
    let invalid = || LocalResponse::err(format!("packages.{subop}: invalid-request"));
    let id = params
        .get("id")
        .or_else(|| params.get("package_id"))
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    let version = params
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    Ok((id, version))
}

/// Package id + optional `version` — KOS-353: callers like the Manager key
/// rows by id alone and omit `version`. The Engine resolves it: the single
/// installed version for that id, else the single catalog-listed version
/// (covers install/disclosure of not-yet-installed packages). An unknown or
/// ambiguous id stays `invalid-request`.
pub(in crate::ws_server) fn package_id_resolve_version(
    subop: &str,
    params: &serde_json::Value,
    service: &PackageService,
) -> Result<(String, String), LocalResponse> {
    let invalid = || LocalResponse::err(format!("packages.{subop}: invalid-request"));
    let id = params
        .get("id")
        .or_else(|| params.get("package_id"))
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if let Some(version) = params
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
    {
        return Ok((id.to_owned(), version.to_owned()));
    }
    let mut versions: Vec<String> = service
        .list()
        .map(|list| {
            list.packages
                .iter()
                .filter(|package| package.id == id)
                .map(|package| package.version.clone())
                .collect()
        })
        .unwrap_or_default();
    if versions.is_empty() {
        if let Ok(catalog) = service.catalog_packages(None) {
            versions = catalog
                .iter()
                .filter(|entry| entry.id == id)
                .map(|entry| entry.version.clone())
                .collect();
        }
    }
    versions.sort();
    versions.dedup();
    if versions.len() == 1 {
        Ok((id.to_owned(), versions.remove(0)))
    } else {
        Err(invalid())
    }
}

pub(in crate::ws_server) async fn package_blocking<T, F>(work: F) -> Result<T, PackageError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, PackageError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| PackageError::Persistence)?
}

pub(in crate::ws_server) fn package_response<T: serde::Serialize>(
    operation: &str,
    result: Result<T, PackageError>,
) -> LocalResponse {
    match result {
        Ok(value) => match serde_json::to_value(value) {
            Ok(value) => LocalResponse::ok(value),
            Err(_) => LocalResponse::err(format!("packages.{operation}: serialization-failed")),
        },
        Err(error) => LocalResponse::err(format!(
            "packages.{operation}: {}",
            package_error_code(&error)
        )),
    }
}

pub(in crate::ws_server) fn package_error_code(error: &PackageError) -> &'static str {
    match error {
        PackageError::CatalogUnavailable => "catalog-unavailable",
        PackageError::Invalid => "invalid-request",
        PackageError::Persistence => "persistence-failed",
        PackageError::Worker(code) => match *code {
            "forbidden" | "invalid-request" | "not-found" | "conflict" | "timeout"
            | "unavailable" => code,
            _ => "unavailable",
        },
        PackageError::Catalog(crate::catalog::CatalogError::Expired) => "catalog-expired",
        PackageError::Catalog(crate::catalog::CatalogError::Replay) => "replay-rejected",
        PackageError::Catalog(_) => "catalog-rejected",
        PackageError::Store(_) => "store-rejected",
        // apps.* outcomes — unreachable via packages.*, mapped for totality.
        PackageError::NotFound => "not-found",
        PackageError::Busy => "conflict",
        PackageError::AppRunning => "app-running",
        PackageError::Offline => "unavailable",
        PackageError::Integrity => "invalid-request",
        PackageError::Unsupported => "unavailable",
    }
}

#[cfg(test)]
mod package_error_code_tests {
    use super::package_error_code;
    use crate::package_service::PackageError;

    #[test]
    fn worker_error_codes_are_allowlisted() {
        assert_eq!(
            package_error_code(&PackageError::Worker("forbidden")),
            "forbidden"
        );
        assert_eq!(
            package_error_code(&PackageError::Worker("secret")),
            "unavailable"
        );
    }
}
