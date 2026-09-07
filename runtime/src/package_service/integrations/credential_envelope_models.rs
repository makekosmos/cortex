use serde::{Deserialize, Serialize};
use std::ops::Deref;

pub(crate) struct DecryptedSecret(pub(crate) String);

impl DecryptedSecret {
    pub(crate) fn as_str(&self) -> &str { &self.0 }
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
pub(crate) struct CredentialContext {
    pub(crate) integration_id: String,
    pub(crate) recipient_node_id: String,
    pub(crate) grant_epoch: u64,
    pub(crate) credential_generation: u64,
    pub(crate) key_id: String,
    pub(crate) issuer_node_id: String,
    pub(crate) issuer_auth_key_id: String,
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
pub(crate) struct CredentialEnvelopeV2 {
    pub(crate) version: u8,
    pub(crate) algorithm: String,
    pub(crate) integration_id: String,
    pub(crate) recipient_node_id: String,
    pub(crate) grant_epoch: u64,
    pub(crate) credential_generation: u64,
    pub(crate) key_id: String,
    pub(crate) issuer_node_id: String,
    pub(crate) issuer_auth_key_id: String,
    pub(crate) enc: String,
    pub(crate) ciphertext: String,
}
