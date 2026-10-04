//! Lease state for the package launch surface: asset grants, launch
//! leases, and the bounded registry with TTL/grace handling.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::auth;
use crate::runtime_grants::LaunchGrant;

use super::{
    package_origin_host, BOOTSTRAP_CODE_TTL, DATA_GRANT_TTL, LAUNCH_LEASE_TTL,
    MAX_ACTIVE_LAUNCH_LEASES, RELEASE_GRACE,
};

#[derive(Debug, Clone)]
pub struct AssetGrant {
    pub id: String,
    pub version: String,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct LaunchLease {
    pub launch_id: String,
    pub asset_token: String,
    pub launch_token: Option<String>,
    /// Single-use code the page exchanges at `/bootstrap` for the launch
    /// token — minted with every typed-grant lease, burned on first use.
    pub bootstrap_code: Option<String>,
    pub bootstrap_expires_at: Option<Instant>,
    pub bootstrap_spent: bool,
    /// Set by the `/release` pagehide beacon: the page is leaving. A `renew`
    /// inside `RELEASE_GRACE` clears this (reload); past it, the lease is
    /// purged like an expiry (real close).
    pub released_at: Option<Instant>,
    pub grant: AssetGrant,
    pub typed_grant: Option<LaunchGrant>,
    pub expires_at: Instant,
    pub expires_at_rfc3339: String,
    pub grant_expires_at: Option<Instant>,
    pub grant_expires_at_rfc3339: Option<String>,
}

#[derive(Debug)]
pub struct LaunchLeaseRegistry {
    leases: HashMap<String, LaunchLease>,
    asset_tokens: HashMap<String, String>,
    pub ttl: Duration,
    capacity: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaseCapacityError;

impl Default for LaunchLeaseRegistry {
    fn default() -> Self {
        Self::with_limits(LAUNCH_LEASE_TTL, MAX_ACTIVE_LAUNCH_LEASES)
    }
}

impl LaunchLeaseRegistry {
    pub fn with_limits(ttl: Duration, capacity: usize) -> Self {
        Self {
            leases: HashMap::new(),
            asset_tokens: HashMap::new(),
            ttl,
            capacity,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn create(&mut self, grant: AssetGrant) -> Result<LaunchLease, LeaseCapacityError> {
        self.try_create_with_grant_at(grant, None, Instant::now())
    }

    pub fn create_with_typed_grant(
        &mut self,
        grant: AssetGrant,
        typed_grant: LaunchGrant,
    ) -> Result<LaunchLease, LeaseCapacityError> {
        self.try_create_with_grant_at(grant, Some(typed_grant), Instant::now())
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn try_create_at(
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
        // Typed-grant leases carry a single-use bootstrap code the served
        // page exchanges for its credentials; untyped leases never do.
        let bootstrap_code = typed_grant.is_some().then(new_asset_token);
        let bootstrap_expires_at = typed_grant.is_some().then_some(now + BOOTSTRAP_CODE_TTL);
        let grant_expires_at = typed_grant.as_ref().map(|_| now + lease_ttl);
        let grant_expires_at_rfc3339 = typed_grant.as_ref().map(|_| {
            (chrono::Utc::now() + chrono::Duration::seconds(lease_ttl.as_secs() as i64))
                .to_rfc3339()
        });
        let lease = LaunchLease {
            launch_id: uuid::Uuid::new_v4().to_string(),
            asset_token: new_asset_token(),
            launch_token,
            bootstrap_code,
            bootstrap_expires_at,
            bootstrap_spent: false,
            released_at: None,
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

    /// Burn the one-time bootstrap code: every failure mode — wrong code,
    /// expired code, reused code, revoked/expired lease, untyped lease —
    /// returns the same `None` so the endpoint is no oracle. The code is
    /// marked spent *before* the lease is cloned out, so even a crashed
    /// caller can never replay it.
    pub fn bootstrap(&mut self, launch_id: &str, code: &str) -> Option<LaunchLease> {
        self.purge_expired();
        let lease = self.leases.get_mut(launch_id)?;
        let stored = lease.bootstrap_code.as_deref()?;
        if lease.bootstrap_spent
            || lease.expires_at <= Instant::now()
            || lease
                .bootstrap_expires_at
                .is_none_or(|expiry| expiry <= Instant::now())
            || !auth::validate_token(code, stored)
        {
            return None;
        }
        lease.bootstrap_spent = true;
        Some(lease.clone())
    }

    /// Mark the lease released (`pagehide` beacon): it keeps working through
    /// `RELEASE_GRACE` so a reloaded page can renew and continue, then is
    /// purged as if expired. Wrong/missing token → `false` — no oracle.
    pub fn release(&mut self, launch_id: &str, token: &str) -> bool {
        self.purge_expired();
        let Some(lease) = self.leases.get_mut(launch_id) else {
            return false;
        };
        let Some(stored) = lease.launch_token.as_deref() else {
            return false;
        };
        if !auth::validate_token(token, stored) {
            return false;
        }
        lease.released_at = Some(Instant::now());
        true
    }

    /// Read-only `typed_grant` for the event stream's per-frame liveness:
    /// token-valid, unexpired, and not released — a closed tab stops
    /// receiving immediately rather than after the TTL.
    pub fn live_grant(&self, launch_id: &str, token: &str) -> Option<(AssetGrant, LaunchGrant)> {
        let lease = self.leases.get(launch_id)?;
        let stored = lease.launch_token.as_deref()?;
        if !auth::validate_token(token, stored)
            || lease.released_at.is_some()
            || lease.expires_at <= Instant::now()
            || lease
                .grant_expires_at
                .is_some_and(|expires_at| expires_at <= Instant::now())
        {
            return None;
        }
        Some((lease.grant.clone(), lease.typed_grant.clone()?))
    }

    /// The exact `Origin` a bootstrap for this lease must come from — the
    /// lease's own package host, so a sibling package's page cannot spend a
    /// code. `None` for unknown leases: the caller denies uniformly.
    pub fn expected_origin(&self, launch_id: &str, http_port: u16) -> Option<String> {
        self.leases.get(launch_id).map(|lease| {
            format!(
                "http://{}:{http_port}",
                package_origin_host(&lease.grant.id)
            )
        })
    }

    /// Immediate revoke by launch credential — used by tests and any
    /// launch-scoped caller that needs a hard kill rather than the grace
    /// window `release` provides.
    pub fn revoke_with_token(&mut self, launch_id: &str, token: &str) -> bool {
        self.purge_expired();
        let Some(lease) = self.leases.get(launch_id) else {
            return false;
        };
        let Some(stored) = lease.launch_token.as_deref() else {
            return false;
        };
        if !auth::validate_token(token, stored) {
            return false;
        }
        self.revoke(launch_id)
    }

    pub fn asset(&mut self, asset_token: &str) -> Option<AssetGrant> {
        self.purge_expired();
        let launch_id = self.asset_tokens.get(asset_token)?;
        self.leases
            .get(launch_id)
            .filter(|lease| lease.expires_at > Instant::now())
            .map(|lease| lease.grant.clone())
    }

    pub fn revoke(&mut self, launch_id: &str) -> bool {
        self.purge_expired();
        let Some(lease) = self.leases.remove(launch_id) else {
            return false;
        };
        self.asset_tokens.remove(&lease.asset_token);
        true
    }

    pub fn typed_grant(
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

    pub fn renew(&mut self, launch_id: &str, launch_token: &str) -> Option<LaunchLease> {
        self.purge_expired();
        let lease = self.leases.get_mut(launch_id)?;
        let token = lease.launch_token.as_deref()?;
        if !auth::validate_token(launch_token, token) || lease.typed_grant.is_none() {
            return None;
        }
        // A renew inside the release grace cancels the pagehide mark — the
        // page reloaded, it did not close.
        lease.released_at = None;
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

    pub fn purge_expired(&mut self) {
        self.purge_expired_at(Instant::now());
    }

    pub fn purge_expired_at(&mut self, now: Instant) {
        let expired = self
            .leases
            .iter()
            .filter_map(|(id, lease)| {
                if lease.expires_at <= now
                    || lease
                        .released_at
                        .is_some_and(|released| released + RELEASE_GRACE <= now)
                {
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn len(&self) -> usize {
        self.leases.len()
    }

    /// Test seam: age the bootstrap code past its TTL without killing the
    /// lease — the served page may outlive its exchange window.
    #[cfg(any(test, feature = "test-support"))]
    pub fn expire_bootstrap(&mut self, launch_id: &str) {
        if let Some(lease) = self.leases.get_mut(launch_id) {
            lease.bootstrap_expires_at = Some(Instant::now() - Duration::from_secs(1));
        }
    }

    /// Test seam: age the release mark past the grace window.
    #[cfg(any(test, feature = "test-support"))]
    pub fn expire_release(&mut self, launch_id: &str) {
        if let Some(lease) = self.leases.get_mut(launch_id) {
            lease.released_at = Some(Instant::now() - RELEASE_GRACE - Duration::from_secs(1));
        }
    }

    /// Test seam: expire one lease in place (`expire_launch_for_test`).
    #[cfg(any(test, feature = "test-support"))]
    pub fn expire(&mut self, launch_id: &str) {
        if let Some(lease) = self.leases.get_mut(launch_id) {
            lease.expires_at = Instant::now() - Duration::from_secs(1);
        }
    }
}

fn new_asset_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
