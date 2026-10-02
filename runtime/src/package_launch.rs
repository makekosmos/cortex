//! Package launch surface — the launch-lease registry and the launch/resolve
//! payloads shared by the Engine HTTP boundary (`/v1/apps/launch`,
//! `/v1/apps/assets`, `/renew`, `/revoke`) and the Manager-facing
//! `packages.open` dispatch op. There is one registry and one mint path: a
//! Manager «Открыть» is the same launch an external package host requests.
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

mod lease;

use crate::runtime_grants::LaunchGrant;

pub(crate) use lease::{AssetGrant, LaunchLease, LaunchLeaseRegistry, LeaseCapacityError};

pub(crate) const LAUNCH_LEASE_TTL: Duration = Duration::from_secs(300);
pub(crate) const DATA_GRANT_TTL: Duration = Duration::from_secs(900);
pub(crate) const MAX_ACTIVE_LAUNCH_LEASES: usize = 2_048;
/// The one-time code the served page exchanges for its launch credentials —
/// short because it rides in the URL fragment, which browsers never send.
pub(crate) const BOOTSTRAP_CODE_TTL: Duration = Duration::from_secs(60);
/// A released lease dies this long after the pagehide beacon. Grace — not
/// instant revoke — lets F5 and back/forward survive: the reloaded page
/// renews inside the window and cancels the release.
pub(crate) const RELEASE_GRACE: Duration = Duration::from_secs(30);

/// Per-package origin host: `p<sha256(id)>.localhost`. Browsers resolve
/// `*.localhost` to loopback, so the Engine keeps listening only on
/// 127.0.0.1 while every package gets a real distinct origin — a shared
/// `127.0.0.1` origin would let any package read every sibling's
/// localStorage/IndexedDB/cookies. The id is hashed so the label stays
/// within DNS limits and cannot collide with a differently-spelled id.
pub(crate) fn package_origin_host(package_id: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(package_id.as_bytes());
    let hex: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!("p{hex}.localhost")
}

/// `POST /v1/apps/launch` and `/v1/apps/resolve` body shape.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LaunchRequest {
    pub id: String,
    pub version: Option<String>,
}

/// The lease registry + bound HTTP port an `EngineApiServer` wires into the
/// package service so `packages.open` mints through the same registry the
/// HTTP routes use. `None` only in engines built without the HTTP surface.
#[derive(Clone)]
pub(crate) struct LaunchSurface {
    pub leases: std::sync::Arc<Mutex<LaunchLeaseRegistry>>,
    pub http_port: u16,
}

pub(crate) fn resolve_payload(package: &crate::package_store::InstalledPackage) -> Value {
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

pub(crate) fn launch_payload(
    http_port: u16,
    lease: &LaunchLease,
    package: &crate::package_store::InstalledPackage,
    ttl: Duration,
) -> Value {
    let mut data = json!({
        "id": package.id,
        "version": package.version,
        "name": package.manifest.name(),
        "launch_url": format!(
            "http://{}:{http_port}/v1/apps/assets/{}/{}",
            package_origin_host(&package.id),
            lease.asset_token,
            package.manifest.entrypoint(),
        ),
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
        let grant = lease.typed_grant.as_ref();
        data["effective_read_types"] = Value::Array(
            grant
                .map(effective_read_types)
                .unwrap_or_default()
                .into_iter()
                .map(Value::String)
                .collect(),
        );
        data["effective_events"] = Value::Array(
            grant
                .map(effective_events)
                .unwrap_or_default()
                .into_iter()
                .map(Value::String)
                .collect(),
        );
    }
    json!({
        "ok": true,
        "data": data
    })
}

/// Type ids an `ark.subscribe` channel may deliver: types whose grant rules
/// allow `subscribe`, plus their canonical aliases — the same filter the
/// Electron host applied with `hasLaunchReadPermission`.
pub(crate) fn effective_read_types(grant: &LaunchGrant) -> Vec<String> {
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
    ids.into_iter().collect()
}

/// Named events the page may subscribe to beyond typed reads.
pub(crate) fn effective_events(grant: &LaunchGrant) -> Vec<String> {
    crate::runtime_grants::AGENTS_READ_OPERATIONS
        .iter()
        .any(|operation| grant.allows_agents_operation(operation))
        .then(|| "agents_event".to_owned())
        .into_iter()
        .collect()
}
