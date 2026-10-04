use serde::{Deserialize, Serialize};
use std::ops::Deref;

pub struct DecryptedSecret(pub String);

impl DecryptedSecret {
    pub fn as_str(&self) -> &str { &self.0 }
}

impl Deref for DecryptedSecret {
    type Target = str;

    fn deref(&self) -> &Self::Target { self.as_str() }
}

impl Drop for DecryptedSecret {
    fn drop(&mut self) {
        crate::package_worker_secrets::zeroize_secret(&mut self.0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialContext {
    pub integration_id: String,
    pub recipient_node_id: String,
    pub grant_epoch: u64,
    pub credential_generation: u64,
    pub key_id: String,
    pub issuer_node_id: String,
    pub issuer_auth_key_id: String,
}

#[derive(Debug, Clone)]
pub struct HpkeIdentity {
    pub node_id: String,
    pub private_key: String,
    pub public_key: String,
    pub key_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CredentialEnvelopeV2 {
    pub version: u8,
    pub algorithm: String,
    pub integration_id: String,
    pub recipient_node_id: String,
    pub grant_epoch: u64,
    pub credential_generation: u64,
    pub key_id: String,
    pub issuer_node_id: String,
    pub issuer_auth_key_id: String,
    pub enc: String,
    pub ciphertext: String,
}
