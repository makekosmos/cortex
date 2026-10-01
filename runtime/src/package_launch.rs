//! Package launch surface — the launch-lease registry and the launch/resolve
//! payloads shared by the Engine HTTP boundary (`/v1/apps/launch`,
//! `/v1/apps/assets`, `/renew`, `/revoke`) and the Manager-facing
//! `packages.open` dispatch op. There is one registry and one mint path: a
//! Manager «Открыть» is the same launch an external package host requests.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth;
use crate::runtime_grants::LaunchGrant;

pub(crate) const LAUNCH_LEASE_TTL: Duration = Duration::from_secs(300);
pub(crate) const DATA_GRANT_TTL: Duration = Duration::from_secs(900);
pub(crate) const MAX_ACTIVE_LAUNCH_LEASES: usize = 2_048;

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

#[derive(Debug, Clone)]
pub(crate) struct AssetGrant {
    pub id: String,
    pub version: String,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub(crate) struct LaunchLease {
    pub launch_id: String,
    pub asset_token: String,
    pub launch_token: Option<String>,
    pub grant: AssetGrant,
    pub typed_grant: Option<LaunchGrant>,
    pub expires_at: Instant,
    pub expires_at_rfc3339: String,
    pub grant_expires_at: Option<Instant>,
    pub grant_expires_at_rfc3339: Option<String>,
}

#[derive(Debug)]
pub(crate) struct LaunchLeaseRegistry {
    leases: HashMap<String, LaunchLease>,
    asset_tokens: HashMap<String, String>,
    pub(crate) ttl: Duration,
    capacity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LeaseCapacityError;

impl Default for LaunchLeaseRegistry {
    fn default() -> Self {
        Self::with_limits(LAUNCH_LEASE_TTL, MAX_ACTIVE_LAUNCH_LEASES)
    }
}

impl LaunchLeaseRegistry {
    pub(crate) fn with_limits(ttl: Duration, capacity: usize) -> Self {
        Self {
            leases: HashMap::new(),
            asset_tokens: HashMap::new(),
            ttl,
            capacity,
        }
    }

    #[cfg(test)]
    pub(crate) fn create(&mut self, grant: AssetGrant) -> Result<LaunchLease, LeaseCapacityError> {
        self.try_create_with_grant_at(grant, None, Instant::now())
    }

    pub(crate) fn create_with_typed_grant(
        &mut self,
        grant: AssetGrant,
        typed_grant: LaunchGrant,
    ) -> Result<LaunchLease, LeaseCapacityError> {
        self.try_create_with_grant_at(grant, Some(typed_grant), Instant::now())
    }

    #[cfg(test)]
    pub(crate) fn try_create_at(
        &mut self,
        grant: AssetGrant,
        now: Instant,
    ) -> Result<LaunchLease, LeaseCapacityError> {
        self.try_create_with_grant_at(grant, None, now)
    }

    fn try_create_with_grant_at(
        &mut self,
        grant: AssetGrant,
        typed_grant: Option<LaunchGrant>,
        now: Instant,
    ) -> Result<LaunchLease, LeaseCapacityError> {
        self.purge_expired_at(now);
        if self.leases.len() >= self.capacity {
            return Err(LeaseCapacityError);
        }
        let launch_token = typed_grant.as_ref().map(|_| new_asset_token());
        let lease_ttl = if typed_grant.is_some() {
            DATA_GRANT_TTL
        } else {
            self.ttl
        };
        let grant_expires_at = typed_grant.as_ref().map(|_| now + lease_ttl);
        let grant_expires_at_rfc3339 = typed_grant.as_ref().map(|_| {
            (chrono::Utc::now() + chrono::Duration::seconds(lease_ttl.as_secs() as i64))
                .to_rfc3339()
        });
        let lease = LaunchLease {
            launch_id: uuid::Uuid::new_v4().to_string(),
            asset_token: new_asset_token(),
            launch_token,
            grant,
            typed_grant,
            expires_at: now + lease_ttl,
            expires_at_rfc3339: (chrono::Utc::now()
                + chrono::Duration::seconds(lease_ttl.as_secs() as i64))
            .to_rfc3339(),
            grant_expires_at,
            grant_expires_at_rfc3339,
        };
        self.asset_tokens
            .insert(lease.asset_token.clone(), lease.launch_id.clone());
        self.leases.insert(lease.launch_id.clone(), lease.clone());
        Ok(lease)
    }

    /// Reuse the live lease for this exact package id+version: `packages.open`
    /// cannot raise the host window, so a repeat open returns the still-valid
    /// session — TTL-extended exactly like `/renew` — instead of minting a
    /// duplicate lease for the same running app.
    pub(crate) fn reopen(&mut self, id: &str, version: &str) -> Option<LaunchLease> {
        self.purge_expired();
        let launch_id = self
            .leases
            .values()
            .find(|lease| {
                lease.grant.id == id
                    && lease.grant.version == version
                    && lease.expires_at > Instant::now()
            })
            .map(|lease| lease.launch_id.clone())?;
        let lease = self.leases.get_mut(&launch_id)?;
        let now = Instant::now();
        lease.expires_at = now + DATA_GRANT_TTL;
        lease.expires_at_rfc3339 = (chrono::Utc::now()
            + chrono::Duration::seconds(DATA_GRANT_TTL.as_secs() as i64))
        .to_rfc3339();
        lease.grant_expires_at = Some(now + DATA_GRANT_TTL);
        lease.grant_expires_at_rfc3339 = Some(lease.expires_at_rfc3339.clone());
        Some(lease.clone())
    }

    pub(crate) fn asset(&mut self, asset_token: &str) -> Option<AssetGrant> {
        self.purge_expired();
        let launch_id = self.asset_tokens.get(asset_token)?;
        self.leases
            .get(launch_id)
            .filter(|lease| lease.expires_at > Instant::now())
            .map(|lease| lease.grant.clone())
    }

    pub(crate) fn revoke(&mut self, launch_id: &str) -> bool {
        self.purge_expired();
        let Some(lease) = self.leases.remove(launch_id) else {
            return false;
        };
        self.asset_tokens.remove(&lease.asset_token);
        true
    }

    pub(crate) fn typed_grant(
        &mut self,
        launch_id: &str,
        launch_token: &str,
    ) -> Option<(AssetGrant, LaunchGrant)> {
        self.purge_expired();
        let lease = self.leases.get(launch_id)?;
        let token = lease.launch_token.as_deref()?;
        if !auth::validate_token(launch_token, token)
            || lease
                .grant_expires_at
                .is_some_and(|expires_at| expires_at <= Instant::now())
        {
            return None;
        }
        Some((lease.grant.clone(), lease.typed_grant.clone()?))
    }

    pub(crate) fn renew(&mut self, launch_id: &str, launch_token: &str) -> Option<LaunchLease> {
        self.purge_expired();
        let lease = self.leases.get_mut(launch_id)?;
        let token = lease.launch_token.as_deref()?;
        if !auth::validate_token(launch_token, token) || lease.typed_grant.is_none() {
            return None;
        }
        let now = Instant::now();
        lease.expires_at = now + DATA_GRANT_TTL;
        lease.expires_at_rfc3339 = (chrono::Utc::now()
            + chrono::Duration::seconds(DATA_GRANT_TTL.as_secs() as i64))
        .to_rfc3339();
        lease.grant_expires_at = Some(now + DATA_GRANT_TTL);
        lease.grant_expires_at_rfc3339 = Some(
            (chrono::Utc::now() + chrono::Duration::seconds(DATA_GRANT_TTL.as_secs() as i64))
                .to_rfc3339(),
        );
        Some(lease.clone())
    }

    pub(crate) fn purge_expired(&mut self) {
        self.purge_expired_at(Instant::now());
    }

    pub(crate) fn purge_expired_at(&mut self, now: Instant) {
        let expired = self
            .leases
            .iter()
            .filter_map(|(id, lease)| {
                if lease.expires_at <= now {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for id in expired {
            self.revoke_expired(&id);
        }
    }

    fn revoke_expired(&mut self, launch_id: &str) {
        if let Some(lease) = self.leases.remove(launch_id) {
            self.asset_tokens.remove(&lease.asset_token);
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.leases.len()
    }

    /// Test seam: expire one lease in place (`expire_launch_for_test`).
    #[cfg(test)]
    pub(crate) fn expire(&mut self, launch_id: &str) {
        if let Some(lease) = self.leases.get_mut(launch_id) {
            lease.expires_at = Instant::now() - Duration::from_secs(1);
        }
    }
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
        let effective_events = lease
            .typed_grant
            .as_ref()
            .is_some_and(|grant| {
                crate::runtime_grants::AGENTS_READ_OPERATIONS
                    .iter()
                    .any(|operation| grant.allows_agents_operation(operation))
            })
            .then(|| Value::String("agents_event".into()))
            .into_iter()
            .collect();
        data["effective_events"] = Value::Array(effective_events);
    }
    json!({
        "ok": true,
        "data": data
    })
}

fn new_asset_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
