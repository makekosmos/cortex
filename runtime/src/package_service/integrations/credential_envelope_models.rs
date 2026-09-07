use serde::{Deserialize, Serialize};

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
