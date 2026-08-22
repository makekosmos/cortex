use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopLease {
    pub session_id: String,
    pub generation: u64,
    pub electron_pid: u32,
    credential_hash: [u8; 32],
    bound_connection: Option<u64>,
}

#[derive(Debug, Default)]
pub struct DesktopAuthorityRegistry {
    leases: Mutex<HashMap<(String, u64), DesktopLease>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityError {
    InvalidCredential,
    WrongPid,
    AlreadyBound,
    MissingLease,
    StaleGeneration,
}

impl DesktopAuthorityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &self,
        session_id: String,
        generation: u64,
        electron_pid: u32,
        credential: &str,
    ) {
        let mut leases = self.leases.lock().unwrap_or_else(|p| p.into_inner());
        leases.retain(|(session, gen), _| session != &session_id || *gen >= generation);
        leases.insert(
            (session_id.clone(), generation),
            DesktopLease {
                session_id,
                generation,
                electron_pid,
                credential_hash: hash_credential(credential),
                bound_connection: None,
            },
        );
    }

    pub fn bind(
        &self,
        session_id: &str,
        generation: u64,
        electron_pid: u32,
        credential: &str,
        connection_id: u64,
    ) -> Result<(), AuthorityError> {
        let mut leases = self.leases.lock().unwrap_or_else(|p| p.into_inner());
        let Some(lease) = leases.get_mut(&(session_id.to_owned(), generation)) else {
            return Err(AuthorityError::MissingLease);
        };
        if lease.electron_pid != electron_pid {
            return Err(AuthorityError::WrongPid);
        }
        if !constant_time_equal(&lease.credential_hash, &hash_credential(credential)) {
            return Err(AuthorityError::InvalidCredential);
        }
        if lease.bound_connection.is_some() {
            return Err(AuthorityError::AlreadyBound);
        }
        lease.bound_connection = Some(connection_id);
        Ok(())
    }

    pub fn disconnect(&self, connection_id: u64) {
        let mut leases = self.leases.lock().unwrap_or_else(|p| p.into_inner());
        for lease in leases.values_mut() {
            if lease.bound_connection == Some(connection_id) {
                lease.bound_connection = None;
            }
        }
    }

    pub fn revoke_generation(&self, session_id: &str, generation: u64) {
        let mut leases = self.leases.lock().unwrap_or_else(|p| p.into_inner());
        leases.retain(|(session, gen), _| session != session_id || *gen != generation);
    }

    pub fn authorize(&self, connection_id: u64) -> bool {
        self.leases
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .any(|lease| lease.bound_connection == Some(connection_id))
    }

    pub fn owner(&self, connection_id: u64) -> Option<(String, u64)> {
        self.leases.lock().ok()?.values().find_map(|lease| {
            (lease.bound_connection == Some(connection_id))
                .then(|| (lease.session_id.clone(), lease.generation))
        })
    }

    pub fn len(&self) -> usize {
        self.leases.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}
fn hash_credential(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_private_credential_binds_once_and_wrong_pid_is_denied() {
        let registry = DesktopAuthorityRegistry::new();
        registry.register("session".into(), 4, 123, "credential");
        assert_eq!(
            registry.bind("session", 4, 999, "credential", 1),
            Err(AuthorityError::WrongPid)
        );
        assert_eq!(
            registry.bind("session", 4, 123, "wrong", 1),
            Err(AuthorityError::InvalidCredential)
        );
        assert_eq!(registry.bind("session", 4, 123, "credential", 1), Ok(()));
        assert_eq!(
            registry.bind("session", 4, 123, "credential", 2),
            Err(AuthorityError::AlreadyBound)
        );
    }

    #[test]
    fn disconnect_clears_binding_but_revoke_removes_lease() {
        let registry = DesktopAuthorityRegistry::new();
        registry.register("session".into(), 1, 123, "old");
        registry.register("session".into(), 2, 123, "new");
        assert_eq!(
            registry.bind("session", 1, 123, "old", 1),
            Err(AuthorityError::MissingLease)
        );
        assert_eq!(registry.bind("session", 2, 123, "new", 2), Ok(()));
        registry.disconnect(2);
        assert_eq!(registry.len(), 1);
        assert!(!registry.authorize(2));
        assert_eq!(registry.bind("session", 2, 123, "new", 3), Ok(()));
        assert!(registry.authorize(3));
        assert_eq!(
            registry.bind("session", 2, 999, "new", 4),
            Err(AuthorityError::WrongPid)
        );
        registry.revoke_generation("session", 2);
        assert_eq!(registry.len(), 0);
        assert_eq!(
            registry.bind("session", 2, 123, "new", 5),
            Err(AuthorityError::MissingLease)
        );
    }

    #[test]
    fn global_token_or_spoofed_class_has_no_authority() {
        let registry = DesktopAuthorityRegistry::new();
        registry.register("session".into(), 1, 123, "private");
        assert_eq!(
            registry.bind("session", 1, 123, "global-token", 1),
            Err(AuthorityError::InvalidCredential)
        );
        assert_eq!(
            registry.bind("session", 1, 123, "kosmos-desktop", 1),
            Err(AuthorityError::InvalidCredential)
        );
    }
}
