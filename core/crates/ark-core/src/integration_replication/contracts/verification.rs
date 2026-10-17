use ark_domain::integration_replication::IntegrationRefreshLease;
use ark_domain::integration_replication::{GrantStatus, NodeStatus};
use serde::{Deserialize, Serialize};

/// Plaintext-free readiness snapshot for the trusted integration host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationVerificationStatus {
    pub integration_id: String,
    pub local_node_id: String,
    pub integration_present: bool,
    pub integration_enabled: Option<bool>,
    pub node_status: Option<NodeStatus>,
    pub node_grant_epoch: Option<u64>,
    pub grant_status: Option<GrantStatus>,
    pub grant_epoch: Option<u64>,
    pub grant_key_matches_node: bool,
    pub envelope_generation: Option<u64>,
    pub envelope_grant_epoch: Option<u64>,
    pub envelope_key_id: Option<String>,
    pub envelope_available: bool,
    pub refresh_lease: Option<IntegrationRefreshLease>,
    pub lease_is_current: bool,
    pub ready_for_collection: bool,
    pub ready_for_refresh: bool,
}

/// Trusted issuer key material for decrypting one addressed credential
/// envelope. No ciphertext or credential data crosses this read contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IssuerEncryptionKey {
    pub schema_version: u8,
    pub space_id: String,
    pub integration_id: String,
    pub recipient_node_id: String,
    pub issuer_node_id: String,
    pub encryption_public_key: String,
    pub key_id: String,
    pub status: NodeStatus,
    pub grant_status: GrantStatus,
    pub grant_epoch: u64,
    pub credential_generation: u64,
    /// Fencing token the published envelope was minted under. The consumer
    /// compares it against the current refresh lease before storing the
    /// credential (`integration.check_credential_fence`), so a superseded
    /// publication cannot be accepted. Carried on the lookup result because
    /// it is authorization state, not ciphertext.
    pub refresh_fencing_token: u64,
}

/// Trusted issuer key material for an addressed credential before its first
/// envelope exists. Core returns public authorization state only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IssuerEncryptionKeyForPublish {
    pub schema_version: u8,
    pub space_id: String,
    pub integration_id: String,
    pub recipient_node_id: String,
    pub issuer_node_id: String,
    pub encryption_public_key: String,
    pub key_id: String,
    pub status: NodeStatus,
    pub grant_status: GrantStatus,
    pub grant_epoch: u64,
}
