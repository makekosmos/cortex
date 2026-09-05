#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationRefreshLease {
    pub integration_id: String,
    pub credential_generation: u64,
    pub holder_node_id: String,
    pub fencing_token: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}

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

impl IntegrationRefreshLease {
    pub fn validate(&self) -> Result<(), IntegrationContractError> {
        require_text(&self.integration_id, "integration_id")?;
        require_text(&self.holder_node_id, "holder_node_id")?;
        require_nonzero(self.credential_generation, "credential_generation")?;
        require_nonzero(self.fencing_token, "fencing_token")?;
        if self.expires_at_ms <= self.issued_at_ms {
            return Err(IntegrationContractError::Mismatch {
                field: "lease_interval",
            });
        }
        Ok(())
    }

    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_at_ms
    }
}
