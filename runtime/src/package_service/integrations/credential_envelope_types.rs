use super::{CredentialEnvelopeV2, HpkeIdentity, ALGORITHM, ENVELOPE_VERSION};
use crate::package_service::PackageError;
use serde_json::Value;
use std::fmt;
use std::str;

impl CredentialEnvelopeV2 {
    pub(crate) fn from_core_value(
        value: &Value,
        issuer_auth_key_id: &str,
    ) -> Result<Self, CredentialEnvelopeError> {
        let object = value
            .as_object()
            .ok_or(CredentialEnvelopeError::InvalidEnvelope)?;
        let string = |field: &str| {
            object
                .get(field)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .ok_or(CredentialEnvelopeError::InvalidEnvelope)
        };
        let envelope = Self {
            version: ENVELOPE_VERSION,
            algorithm: string("algorithm")?,
            integration_id: string("integration_id")?,
            recipient_node_id: string("recipient_node_id")?,
            grant_epoch: object
                .get("grant_epoch")
                .and_then(Value::as_u64)
                .ok_or(CredentialEnvelopeError::InvalidEnvelope)?,
            credential_generation: object
                .get("credential_generation")
                .and_then(Value::as_u64)
                .ok_or(CredentialEnvelopeError::InvalidEnvelope)?,
            key_id: string("key_id")?,
            issuer_node_id: string("issuer_node_id")?,
            issuer_auth_key_id: issuer_auth_key_id.to_owned(),
            enc: string("nonce")?,
            ciphertext: string("ciphertext")?,
        };
        if envelope.algorithm != ALGORITHM {
            return Err(CredentialEnvelopeError::InvalidEnvelope);
        }
        Ok(envelope)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialEnvelopeError {
    #[error("invalid credential envelope")]
    InvalidEnvelope,
    #[error("invalid credential envelope encoding")]
    InvalidEncoding,
    #[error("credential envelope authentication failed")]
    Authentication,
    #[error("credential envelope persistence failed")]
    Persistence,
    #[error("credential envelope plaintext is not UTF-8")]
    InvalidPlaintext,
    #[error("credential envelope HPKE operation failed")]
    Hpke,
}

impl From<PackageError> for CredentialEnvelopeError {
    fn from(_: PackageError) -> Self {
        Self::Persistence
    }
}

impl fmt::Display for HpkeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HpkeIdentity")
            .field("node_id", &self.node_id)
            .field("public_key", &self.public_key)
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}
