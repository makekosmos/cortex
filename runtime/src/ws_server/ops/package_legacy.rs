use super::*;

pub(super) fn restore_legacy_grants(
    subop: &str,
    params: serde_json::Value,
    service: &Arc<PackageService>,
) -> LocalResponse {
    if let Some(token) = params.get("transaction_token").and_then(Value::as_str) {
        return package_response(
            subop,
            service
                .restore_legacy_grants(token)
                .map(|restored| serde_json::json!({ "restored": restored })),
        );
    }
    let Some(source_ids) = params.get("source_ids").and_then(Value::as_array) else {
        return LocalResponse::err("packages.restore_legacy_grants: invalid-request");
    };
    let Some(source_ids) = source_ids
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()
    else {
        return LocalResponse::err("packages.restore_legacy_grants: invalid-request");
    };
    let source_ids = source_ids
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    package_response(
        subop,
        service
            .restore_legacy_grants_for_sources(&source_ids)
            .map(|restored| serde_json::json!({ "restored": restored })),
    )
}
