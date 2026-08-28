fn resolve_payload(package: &crate::package_store::InstalledPackage) -> Value {
    json!({
        "ok": true,
        "data": {
            "id": package.id,
            "version": package.version,
            "name": package.manifest.name(),
            "permissions": package.manifest.permissions(),
            "enabled": package.enabled,
            "revoked": package.revoked,
        }
    })
}

fn launch_payload(
    http_port: u16,
    lease: &LaunchLease,
    package: &crate::package_store::InstalledPackage,
    ttl: Duration,
) -> Value {
    let mut data = json!({
        "id": package.id,
        "version": package.version,
        "name": package.manifest.name(),
        "launch_url": format!("http://127.0.0.1:{http_port}/v1/apps/assets/{}/{}", lease.asset_token, package.manifest.entrypoint()),
        "permissions": package.manifest.permissions(),
        "launch_id": lease.launch_id,
        "asset_token": lease.asset_token,
        "ttl_seconds": ttl.as_secs(),
        "expires_at": lease.expires_at_rfc3339,
    });
    if let Some(token) = lease.launch_token.as_ref() {
        data["broker_token"] = Value::String(token.clone());
        data["data_api"] = Value::String(format!(
            "http://127.0.0.1:{http_port}/v1/apps/launch/{}/ark",
            lease.launch_id
        ));
        data["ttl_seconds"] = Value::from(DATA_GRANT_TTL.as_secs());
        if let Some(expires_at) = lease.grant_expires_at_rfc3339.as_ref() {
            data["expires_at"] = Value::String(expires_at.clone());
        }
        data["manifest_schema_version"] = Value::from(2);
        let effective_read_types = lease
            .typed_grant
            .as_ref()
            .map(|grant| {
                let mut ids = std::collections::BTreeSet::new();
                for rule in grant
                    .rules
                    .iter()
                    .filter(|rule| rule.actions.contains("subscribe"))
                {
                    ids.insert(rule.type_id.clone());
                    if let Ok(registrations) =
                        ark_core::canonical_types::definitions::canonical_type_registrations()
                    {
                        if let Some(registration) = registrations
                            .into_iter()
                            .find(|registration| registration.type_id == rule.type_id)
                        {
                            ids.extend(registration.aliases.into_iter().map(|alias| alias.alias));
                        }
                    }
                }
                ids.into_iter().map(Value::String).collect::<Vec<_>>()
            })
            .unwrap_or_default();
        data["effective_read_types"] = Value::Array(effective_read_types);
    }
    json!({
        "ok": true,
        "data": data
    })
}
