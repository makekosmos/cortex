use super::policy::{require_newer, require_nonzero, require_text};
use super::validate_public_settings;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

const DEFAULT_PUBLIC_SETTINGS: fn() -> Value = || Value::Object(Map::new());

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IntegrationContractError {
    #[error("{field} must not be empty")]
    EmptyField { field: &'static str },
    #[error("public settings must be a JSON object")]
    PublicSettingsMustBeObject,
    #[error("sensitive field at {path} is not allowed in public configuration")]
    SensitiveField { path: String },
    #[error("{field} must be greater than zero")]
    ZeroValue { field: &'static str },
    #[error("{field} must be greater than the current value")]
    NonMonotonic { field: &'static str },
    #[error("{field} does not match the expected value")]
    Mismatch { field: &'static str },
    #[error("node is revoked")]
    RevokedNode,
    #[error("grant is revoked")]
    RevokedGrant,
    #[error("refresh lease is expired")]
    ExpiredLease,
    #[error("refresh lease holder does not match the envelope issuer")]
    LeaseHolderMismatch,
    #[error("refresh generation does not match the lease")]
    LeaseGenerationMismatch,
    #[error("refresh fencing token does not match the lease")]
    LeaseFenceMismatch,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationConfiguration {
    pub integration_id: String,
    pub provider: String,
    pub account_subject: String,
    #[serde(default)]
    pub public_scopes: Vec<String>,
    #[serde(default = "default_public_settings")]
    pub public_settings: Value,
    pub enabled: bool,
    /// Non-secret monotonic progress only. Opaque provider cursors belong in
    /// the addressed credential envelope because they may be bearer tokens.
    #[serde(default)]
    pub sync_cursor: Option<u64>,
    pub revision: u64,
    pub hlc: String,
}
fn default_public_settings() -> Value {
    DEFAULT_PUBLIC_SETTINGS()
}
impl IntegrationConfiguration {
    pub fn validate(&self) -> Result<(), IntegrationContractError> {
        require_text(&self.integration_id, "integration_id")?;
        require_text(&self.provider, "provider")?;
        require_text(&self.account_subject, "account_subject")?;
        require_text(&self.hlc, "hlc")?;
        validate_public_settings(&self.public_settings)
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Active,
    Revoked,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorizedNode {
    pub node_id: String,
    pub key_fingerprint: String,
    pub signing_public_key: String,
    pub encryption_public_key: String,
    /// Stable public identity of the point-to-point transport (for example an
    /// Iroh EndpointId). `None` keeps addressed integration traffic disabled
    /// on transports that cannot authenticate a connection-specific peer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_public_key: Option<String>,
    pub grant_epoch: u64,
    pub status: NodeStatus,
    pub authorized_at: String,
    pub revoked_at: Option<String>,
    pub revocation_epoch: Option<u64>,
    pub revision: u64,
    pub hlc: String,
}
impl AuthorizedNode {
    pub fn validate(&self) -> Result<(), IntegrationContractError> {
        require_text(&self.node_id, "node_id")?;
        require_text(&self.key_fingerprint, "key_fingerprint")?;
        require_text(&self.signing_public_key, "signing_public_key")?;
        require_text(&self.encryption_public_key, "encryption_public_key")?;
        if let Some(value) = self.transport_public_key.as_deref() {
            require_text(value, "transport_public_key")?;
        }
        require_text(&self.authorized_at, "authorized_at")?;
        require_text(&self.hlc, "hlc")?;
        require_nonzero(self.grant_epoch, "grant_epoch")?;
        if self.status == NodeStatus::Revoked {
            if self.revoked_at.is_none() || self.revocation_epoch.is_none() {
                return Err(IntegrationContractError::Mismatch {
                    field: "revocation_data",
                });
            }
        } else if self.revoked_at.is_some() || self.revocation_epoch.is_some() {
            return Err(IntegrationContractError::Mismatch {
                field: "revocation_data",
            });
        }
        Ok(())
    }

    pub fn revoke(
        &self,
        revocation_epoch: u64,
        revoked_at: impl Into<String>,
    ) -> Result<Self, IntegrationContractError> {
        self.validate()?;
        if self.status == NodeStatus::Revoked {
            return Err(IntegrationContractError::RevokedNode);
        }
        require_newer(revocation_epoch, self.grant_epoch, "grant_epoch")?;
        let mut next = self.clone();
        next.status = NodeStatus::Revoked;
        next.revoked_at = Some(revoked_at.into());
        next.revocation_epoch = Some(revocation_epoch);
        next.grant_epoch = revocation_epoch;
        next.revision = self.revision.saturating_add(1);
        next.validate()?;
        Ok(next)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reauthorize(
        &self,
        grant_epoch: u64,
        key_fingerprint: impl Into<String>,
        signing_public_key: impl Into<String>,
        encryption_public_key: impl Into<String>,
        transport_public_key: Option<String>,
        authorized_at: impl Into<String>,
        hlc: impl Into<String>,
    ) -> Result<Self, IntegrationContractError> {
        self.validate()?;
        require_newer(grant_epoch, self.grant_epoch, "grant_epoch")?;
        let key_fingerprint = key_fingerprint.into();
        let encryption_public_key = encryption_public_key.into();
        if key_fingerprint == self.key_fingerprint {
            return Err(IntegrationContractError::Mismatch {
                field: "key_fingerprint",
            });
        }
        if encryption_public_key == self.encryption_public_key {
            return Err(IntegrationContractError::Mismatch {
                field: "encryption_public_key",
            });
        }
        let next = Self {
            node_id: self.node_id.clone(),
            key_fingerprint,
            signing_public_key: signing_public_key.into(),
            encryption_public_key,
            transport_public_key,
            grant_epoch,
            status: NodeStatus::Active,
            authorized_at: authorized_at.into(),
            revoked_at: None,
            revocation_epoch: None,
            revision: self.revision.saturating_add(1),
            hlc: hlc.into(),
        };
        next.validate()?;
        Ok(next)
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrantStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationNodeGrant {
    pub integration_id: String,
    pub node_id: String,
    pub node_encryption_key: String,
    pub grant_epoch: u64,
    pub status: GrantStatus,
    pub authorized_at: String,
    pub revoked_at: Option<String>,
    pub revision: u64,
    pub hlc: String,
}
impl IntegrationNodeGrant {
    pub fn validate(&self) -> Result<(), IntegrationContractError> {
        require_text(&self.integration_id, "integration_id")?;
        require_text(&self.node_id, "node_id")?;
        require_text(&self.node_encryption_key, "node_encryption_key")?;
        require_text(&self.authorized_at, "authorized_at")?;
        require_text(&self.hlc, "hlc")?;
        require_nonzero(self.grant_epoch, "grant_epoch")?;
        if self.status == GrantStatus::Revoked && self.revoked_at.is_none() {
            return Err(IntegrationContractError::Mismatch {
                field: "revoked_at",
            });
        }
        if self.status == GrantStatus::Active && self.revoked_at.is_some() {
            return Err(IntegrationContractError::Mismatch {
                field: "revoked_at",
            });
        }
        Ok(())
    }

    pub fn revoke(
        &self,
        revocation_epoch: u64,
        revoked_at: impl Into<String>,
    ) -> Result<Self, IntegrationContractError> {
        self.validate()?;
        if self.status == GrantStatus::Revoked {
            return Err(IntegrationContractError::RevokedGrant);
        }
        require_newer(revocation_epoch, self.grant_epoch, "grant_epoch")?;
        let mut next = self.clone();
        next.status = GrantStatus::Revoked;
        next.grant_epoch = revocation_epoch;
        next.revoked_at = Some(revoked_at.into());
        next.revision = self.revision.saturating_add(1);
        next.validate()?;
        Ok(next)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IntegrationCredentialEnvelope {
    pub integration_id: String,
    pub recipient_node_id: String,
    pub grant_epoch: u64,
    pub credential_generation: u64,
    pub refresh_fencing_token: u64,
    /// `sha256:<lowercase hex>` of the exact UTF-8 bytes of the recipient's
    /// stored encryption public-key string.
    pub key_id: String,
    pub algorithm: String,
    pub nonce: String,
    /// Opaque ciphertext. Core must never add a plaintext credential field.
    pub ciphertext: String,
    #[serde(default)]
    pub authenticated_metadata: Option<Value>,
    pub issuer_node_id: String,
    pub issued_at: String,
    pub revision: u64,
    pub hlc: String,
}
impl IntegrationCredentialEnvelope {
    pub fn validate(&self) -> Result<(), IntegrationContractError> {
        require_text(&self.integration_id, "integration_id")?;
        require_text(&self.recipient_node_id, "recipient_node_id")?;
        require_text(&self.key_id, "key_id")?;
        require_text(&self.algorithm, "algorithm")?;
        require_text(&self.nonce, "nonce")?;
        require_text(&self.ciphertext, "ciphertext")?;
        require_text(&self.issuer_node_id, "issuer_node_id")?;
        require_text(&self.issued_at, "issued_at")?;
        require_text(&self.hlc, "hlc")?;
        let parsed_hlc = crate::hlc::HLC::from_string(&self.hlc);
        if parsed_hlc.device_id != self.issuer_node_id {
            return Err(IntegrationContractError::Mismatch {
                field: "hlc_origin",
            });
        }
        let canonical_wall_time =
            chrono::DateTime::parse_from_rfc3339(&parsed_hlc.wall_time).map(|value| {
                value
                    .with_timezone(&chrono::Utc)
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string()
            });
        if canonical_wall_time.as_deref().ok() != Some(parsed_hlc.wall_time.as_str())
            || parsed_hlc.to_string() != self.hlc
        {
            return Err(IntegrationContractError::Mismatch { field: "hlc" });
        }
        if let Some(metadata) = &self.authenticated_metadata {
            validate_public_settings(metadata)?;
        }
        require_nonzero(self.grant_epoch, "grant_epoch")?;
        require_nonzero(self.credential_generation, "credential_generation")?;
        require_nonzero(self.refresh_fencing_token, "refresh_fencing_token")
    }
}
include!("contracts_part02.rs");
