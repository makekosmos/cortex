use super::*;
pub(in crate::ws_server) fn package_signatures(
    params: &serde_json::Value,
) -> Result<SignatureSet, LocalResponse> {
    params
        .get("signatures")
        .cloned()
        .ok_or_else(|| LocalResponse::err("packages: invalid-request"))
        .and_then(|value| {
            serde_json::from_value(value)
                .map_err(|_| LocalResponse::err("packages: invalid-request"))
        })
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
        PackageError::TrustUnavailable => "trust-unavailable",
        PackageError::Invalid => "invalid-request",
        PackageError::Persistence => "persistence-failed",
        PackageError::Trust(TrustError::Expired) => "catalog-expired",
        PackageError::Trust(TrustError::Replay) => "replay-rejected",
        PackageError::Trust(TrustError::RevokedKey | TrustError::RevokedPackage) => "revoked",
        PackageError::Trust(_) => "trust-rejected",
        PackageError::Store(_) => "store-rejected",
    }
}
