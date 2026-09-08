use super::*;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MigrationSnapshotRequest {
    schema_version: u32,
    target_id: String,
    source_ids: Vec<String>,
    transaction_token: Option<String>,
}

fn migration_sources_match_target(target_id: &str, source_ids: &[String]) -> bool {
    let allowed = match target_id {
        "com.kosmos.arcadia" => &["arcadia", "arrancador"][..],
        "com.kosmos.memoria" => &["eden"][..],
        "com.kosmos.agenda" => &["delphi"][..],
        _ => return false,
    };
    !source_ids.is_empty()
        && source_ids.iter().all(|id| allowed.contains(&id.as_str()))
        && source_ids
            .iter()
            .enumerate()
            .all(|(index, id)| !source_ids[..index].contains(id))
}

pub(crate) fn restore_migration_snapshot(
    subop: &str,
    params: serde_json::Value,
    service: &Arc<PackageService>,
) -> LocalResponse {
    let request: MigrationSnapshotRequest =
        match serde_json::from_value::<MigrationSnapshotRequest>(params) {
            Ok(request) if request.schema_version == 1 => request,
            Ok(_) => {
                return LocalResponse::err("packages.restore_migration_snapshot: invalid-schema")
            }
            Err(_) => {
                return LocalResponse::err("packages.restore_migration_snapshot: invalid-request")
            }
        };
    if !migration_sources_match_target(&request.target_id, &request.source_ids)
        || request
            .transaction_token
            .as_deref()
            .is_some_and(|token| token.is_empty() || token.len() > 128)
    {
        return LocalResponse::err("packages.restore_migration_snapshot: invalid-request");
    }
    package_response(
        subop,
        service
            .restore_migration_snapshot(&request.source_ids, request.transaction_token.as_deref())
            .map(|result| {
                serde_json::json!({
                    "snapshot_found": result.snapshot_found,
                    "restored": result.restored,
                })
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::migration_sources_match_target;

    #[test]
    fn snapshot_source_set_is_typed_and_target_bound() {
        assert!(migration_sources_match_target(
            "com.kosmos.arcadia",
            &["arcadia".into(), "arrancador".into()]
        ));
        assert!(migration_sources_match_target(
            "com.kosmos.arcadia",
            &["arcadia".into()]
        ));
        assert!(!migration_sources_match_target(
            "com.kosmos.arcadia",
            &["eden".into()]
        ));
        assert!(!migration_sources_match_target("com.kosmos.arcadia", &[]));
        assert!(!migration_sources_match_target(
            "com.kosmos.arcadia",
            &["arcadia".into(), "arcadia".into()]
        ));
    }
}
