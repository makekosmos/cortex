use super::*;
use rand::RngCore;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GrantIdentity {
    pub grant_id: String,
    pub launch_id: String,
    pub package_id: String,
    pub package_version: String,
    pub manifest_digest: String,
    pub generation: u64,
    pub boot_epoch: [u8; 32],
}
impl GrantIdentity {
    pub fn new(g: &str, l: &str, p: &str, v: &str, d: &str, n: u64) -> Self {
        Self {
            grant_id: g.into(),
            launch_id: l.into(),
            package_id: p.into(),
            package_version: v.into(),
            manifest_digest: d.into(),
            generation: n,
            boot_epoch: [0; 32],
        }
    }
    pub fn with_epoch(mut self, e: [u8; 32]) -> Self {
        self.boot_epoch = e;
        self
    }
}
#[derive(Clone)]
pub struct TestClock(Arc<Mutex<u64>>);
impl TestClock {
    pub fn new(v: u64) -> Self {
        Self(Arc::new(Mutex::new(v)))
    }
    pub fn set(&self, v: u64) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = v
    }
    fn now(&self) -> u64 {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
#[derive(Clone)]
struct Stored {
    identity: GrantIdentity,
    expires: u64,
    revoked: bool,
}
pub struct GrantStore {
    clock: TestClock,
    max_global: usize,
    max_package: usize,
    epoch: [u8; 32],
    grants: Mutex<HashMap<String, Stored>>,
}
impl GrantStore {
    pub fn with_limits(clock: TestClock, g: usize, p: usize) -> Self {
        Self {
            clock,
            max_global: g,
            max_package: p,
            epoch: [0; 32],
            grants: Mutex::new(HashMap::new()),
        }
    }
    pub fn with_epoch(mut self, e: [u8; 32]) -> Self {
        self.epoch = e;
        self
    }
    pub fn mint(&self, mut id: GrantIdentity, expires: u64) -> Result<GrantIdentity, GrantError> {
        let mut m = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        let now = self.clock.now();
        m.retain(|_, x| x.expires > now && !x.revoked);
        if m.len() >= self.max_global
            || m.values()
                .filter(|x| x.identity.package_id == id.package_id)
                .count()
                >= self.max_package
        {
            return Err(GrantError::Capacity);
        };
        id.boot_epoch = self.epoch;
        m.insert(
            id.launch_id.clone(),
            Stored {
                identity: id.clone(),
                expires,
                revoked: false,
            },
        );
        Ok(id)
    }
    pub fn renew(
        &self,
        launch: &str,
        generation: u64,
        expires: u64,
    ) -> Result<GrantIdentity, GrantError> {
        let mut m = self.grants.lock().unwrap_or_else(|e| e.into_inner());
        let x = m.get_mut(launch).ok_or(GrantError::Missing)?;
        if x.revoked {
            return Err(GrantError::Revoked);
        }
        if x.identity.boot_epoch != self.epoch {
            return Err(GrantError::Revoked);
        }
        if x.expires <= self.clock.now() {
            return Err(GrantError::Expired);
        }
        if x.identity.generation != generation {
            return Err(GrantError::StaleGeneration);
        }
        x.identity.generation += 1;
        x.expires = expires;
        Ok(x.identity.clone())
    }
    pub fn revoke(&self, l: &str) -> bool {
        self.grants
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(l)
            .is_some()
    }
    pub fn active_count(&self) -> usize {
        self.grants.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

pub fn boot_epoch() -> Result<[u8; 32], GrantError> {
    let mut x = [0; 32];
    rand::thread_rng().fill_bytes(&mut x);
    Ok(x)
}
pub fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|x| x.as_secs())
        .unwrap_or(0)
}
