//! Host-owned handles for package-worker secrets.

use rand::RngCore;
use std::{collections::HashMap, fmt, sync::Mutex};

pub const MAX_SECRET_HANDLES: usize = 1024;
const HANDLE_BYTES: usize = 32;
const MAX_BINDING_BYTES: usize = 256;
const MAX_SECRET_BYTES: usize = 64 * 1024;
const HANDLE_ATTEMPTS: usize = 8;

/// An opaque capability for a package-worker setting secret.
///
/// This type intentionally has no serde implementation and does not contain
/// the secret value.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecretHandle([u8; HANDLE_BYTES]);

impl fmt::Debug for SecretHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretHandle(<opaque>)")
    }
}

impl SecretHandle {
    pub fn token(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn parse(token: &str) -> Option<Self> {
        if token.len() != HANDLE_BYTES * 2 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let mut bytes = [0; HANDLE_BYTES];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&token[index * 2..index * 2 + 2], 16).ok()?;
        }
        Some(Self(bytes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretRegistryError {
    InvalidBinding,
    SecretTooLarge,
    Capacity,
    HandleGeneration,
    NotFound,
    OwnerMismatch,
}

struct SecretRecord {
    package_id: String,
    version: String,
    generation: u64,
    setting_key: String,
    secret: String,
}

impl SecretRecord {
    fn zeroize(&mut self) {
        zeroize_secret(&mut self.secret);
    }
}

pub(crate) fn zeroize_secret(value: &mut String) {
    // Volatile writes keep the clearing operation observable without a
    // zeroization dependency. Zero bytes preserve String's UTF-8 invariant.
    for byte in unsafe { value.as_mut_vec() } {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
}

impl Drop for SecretRecord {
    fn drop(&mut self) {
        self.zeroize();
    }
}

pub struct PackageWorkerSecretRegistry {
    records: Mutex<HashMap<SecretHandle, SecretRecord>>,
}

impl Default for PackageWorkerSecretRegistry {
    fn default() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }
}

impl PackageWorkerSecretRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn issue(
        &self,
        package_id: &str,
        version: &str,
        generation: u64,
        setting_key: &str,
        secret: String,
    ) -> Result<SecretHandle, SecretRegistryError> {
        let mut secret = secret;
        if [package_id, version, setting_key]
            .into_iter()
            .any(|value| value.is_empty() || value.len() > MAX_BINDING_BYTES)
        {
            zeroize_secret(&mut secret);
            return Err(SecretRegistryError::InvalidBinding);
        }
        if secret.len() > MAX_SECRET_BYTES {
            zeroize_secret(&mut secret);
            return Err(SecretRegistryError::SecretTooLarge);
        }

        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if records.len() >= MAX_SECRET_HANDLES {
            zeroize_secret(&mut secret);
            return Err(SecretRegistryError::Capacity);
        }
        let mut rng = rand::rngs::OsRng;
        for _ in 0..HANDLE_ATTEMPTS {
            let mut bytes = [0; HANDLE_BYTES];
            rng.fill_bytes(&mut bytes);
            let handle = SecretHandle(bytes);
            if let std::collections::hash_map::Entry::Vacant(slot) = records.entry(handle) {
                slot.insert(SecretRecord {
                    package_id: package_id.to_owned(),
                    version: version.to_owned(),
                    generation,
                    setting_key: setting_key.to_owned(),
                    secret,
                });
                return Ok(handle);
            }
        }
        zeroize_secret(&mut secret);
        Err(SecretRegistryError::HandleGeneration)
    }

    pub fn resolve(
        &self,
        handle: &SecretHandle,
        package_id: &str,
        version: &str,
        generation: u64,
        setting_key: &str,
    ) -> Result<String, SecretRegistryError> {
        let records = self
            .records
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let record = records.get(handle).ok_or(SecretRegistryError::NotFound)?;
        if record.package_id != package_id
            || record.version != version
            || record.generation != generation
            || record.setting_key != setting_key
        {
            return Err(SecretRegistryError::OwnerMismatch);
        }
        Ok(record.secret.clone())
    }

    pub fn resolve_token(
        &self,
        token: &str,
        package_id: &str,
        version: &str,
        generation: u64,
    ) -> Result<(String, String), SecretRegistryError> {
        let handle = SecretHandle::parse(token).ok_or(SecretRegistryError::NotFound)?;
        let records = self
            .records
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let record = records.get(&handle).ok_or(SecretRegistryError::NotFound)?;
        if record.package_id != package_id
            || record.version != version
            || record.generation != generation
        {
            return Err(SecretRegistryError::OwnerMismatch);
        }
        Ok((record.setting_key.clone(), record.secret.clone()))
    }

    pub fn revoke_generation(&self, package_id: &str, generation: u64) -> usize {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let before = records.len();
        records
            .retain(|_, record| record.package_id != package_id || record.generation != generation);
        before - records.len()
    }

    pub fn revoke_package(&self, package_id: &str) -> usize {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let before = records.len();
        records.retain(|_, record| record.package_id != package_id);
        before - records.len()
    }

    pub fn len(&self) -> usize {
        self.records
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .len()
    }
}

#[cfg(test)]
#[path = "package_worker_secrets/tests.rs"]
mod tests;
