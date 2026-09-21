use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::signed_sync_encoding::{canonical_signing_bytes, decode_fixed};
use super::{AuthorizedNode, IntegrationReplicationChange, NodeStatus};

const SAVEPOINT: &str = "ark_signed_sync_replay";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SignedSyncError {
    #[error("{field} must not be empty")]
    EmptyField { field: &'static str },
    #[error("signed sync frame belongs to a different space")]
    WrongSpace,
    #[error("signed sync origin does not match the authorized node")]
    OriginMismatch,
    #[error("signed sync transport identity does not match the authorized node")]
    TransportMismatch,
    #[error("signed sync origin is not authorized in this database")]
    UnknownOrigin,
    #[error("signed sync recipient does not match the authorized node")]
    RecipientMismatch,
    #[error("signed sync recipient is not active in this database")]
    UnauthorizedRecipient,
    #[error("node is revoked")]
    RevokedNode,
    #[error("signed sync key epoch is stale")]
    StaleEpoch,
    #[error("signed sync key epoch is not current")]
    EpochMismatch,
    #[error("{field} must be lowercase hexadecimal")]
    InvalidEncoding { field: &'static str },
    #[error("signed sync signature does not verify")]
    SignatureMismatch,
    #[error("signed sync payload apply failed: {0}")]
    Apply(String),
    #[error("signed sync message has already been reserved")]
    Replay,
    #[error("signed sync storage failed: {0}")]
    Storage(String),
    #[error("signed sync payload serialization failed: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SignedSyncEnvelope {
    pub space_id: String,
    pub origin_node_id: String,
    pub recipient_node_id: String,
    pub key_epoch: u64,
    pub message_id: String,
    pub payload: Vec<IntegrationReplicationChange>,
    /// Lowercase hex Ed25519 signature over `canonical_signing_bytes`.
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedSyncEnvelope {
    pub space_id: String,
    /// The authenticated outer origin. Inner records never carry transport authority.
    pub origin_node_id: String,
    pub recipient_node_id: String,
    pub key_epoch: u64,
    pub message_id: String,
    pub payload: Vec<IntegrationReplicationChange>,
}

impl SignedSyncEnvelope {
    pub fn new(
        space_id: impl Into<String>,
        origin_node_id: impl Into<String>,
        recipient_node_id: impl Into<String>,
        key_epoch: u64,
        message_id: impl Into<String>,
        payload: Vec<IntegrationReplicationChange>,
        signature: impl Into<String>,
    ) -> Self {
        Self {
            space_id: space_id.into(),
            origin_node_id: origin_node_id.into(),
            recipient_node_id: recipient_node_id.into(),
            key_epoch,
            message_id: message_id.into(),
            payload,
            signature: signature.into(),
        }
    }

    /// Returns the exact bytes a node must sign. No private key enters Core.
    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, SignedSyncError> {
        for change in &self.payload {
            change
                .validate()
                .map_err(|error| SignedSyncError::Serialization(error.to_string()))?;
        }
        canonical_signing_bytes(
            &self.space_id,
            &self.origin_node_id,
            &self.recipient_node_id,
            self.key_epoch,
            &self.message_id,
            &self.payload,
        )
    }

    pub fn verify(
        &self,
        expected_space_id: &str,
        expected_recipient_node_id: &str,
        node: &AuthorizedNode,
    ) -> Result<VerifiedSyncEnvelope, SignedSyncError> {
        require_text(&self.space_id, "space_id")?;
        require_text(&self.origin_node_id, "origin_node_id")?;
        require_text(&self.recipient_node_id, "recipient_node_id")?;
        require_text(&self.message_id, "message_id")?;
        require_text(expected_space_id, "expected_space_id")?;
        require_text(expected_recipient_node_id, "expected_recipient_node_id")?;
        if self.payload.is_empty() {
            return Err(SignedSyncError::EmptyField { field: "payload" });
        }
        for change in &self.payload {
            change
                .validate()
                .map_err(|error| SignedSyncError::Serialization(error.to_string()))?;
        }
        node.validate()
            .map_err(|error| SignedSyncError::Serialization(error.to_string()))?;
        if self.space_id != expected_space_id {
            return Err(SignedSyncError::WrongSpace);
        }
        if self.origin_node_id != node.node_id {
            return Err(SignedSyncError::OriginMismatch);
        }
        if self.recipient_node_id != expected_recipient_node_id {
            return Err(SignedSyncError::RecipientMismatch);
        }
        if node.status == NodeStatus::Revoked {
            return Err(SignedSyncError::RevokedNode);
        }
        if self.key_epoch < node.grant_epoch {
            return Err(SignedSyncError::StaleEpoch);
        }
        if self.key_epoch != node.grant_epoch {
            return Err(SignedSyncError::EpochMismatch);
        }

        let public_key = decode_fixed::<32>(&node.signing_public_key, "signing_public_key")?;
        let signature = decode_fixed::<64>(&self.signature, "signature")?;
        let key = VerifyingKey::from_bytes(&public_key).map_err(|_| {
            SignedSyncError::InvalidEncoding {
                field: "signing_public_key",
            }
        })?;
        let signature = Signature::from_bytes(&signature);
        key.verify(&self.canonical_signing_bytes()?, &signature)
            .map_err(|_| SignedSyncError::SignatureMismatch)?;

        Ok(VerifiedSyncEnvelope {
            space_id: self.space_id.clone(),
            origin_node_id: self.origin_node_id.clone(),
            recipient_node_id: self.recipient_node_id.clone(),
            key_epoch: self.key_epoch,
            message_id: self.message_id.clone(),
            payload: self.payload.clone(),
        })
    }
}

/// Verify, reserve, and apply a frame in one savepoint.
pub fn verify_reserve_and_apply_signed_sync<T>(
    conn: &Connection,
    envelope: &SignedSyncEnvelope,
    expected_space_id: &str,
    expected_recipient_node_id: &str,
    node: &AuthorizedNode,
    reserved_at_ms: u64,
    apply: impl FnOnce(&Connection, &VerifiedSyncEnvelope) -> Result<T, String>,
) -> Result<T, SignedSyncError> {
    let verified = envelope.verify(expected_space_id, expected_recipient_node_id, node)?;
    let reserved_at_ms = i64::try_from(reserved_at_ms).map_err(|_| {
        SignedSyncError::Storage("reserved_at_ms exceeds SQLite INTEGER range".into())
    })?;
    let key_epoch = i64::try_from(verified.key_epoch)
        .map_err(|_| SignedSyncError::Storage("key_epoch exceeds SQLite INTEGER range".into()))?;
    conn.execute_batch(&format!("SAVEPOINT {SAVEPOINT}"))
        .map_err(|error| SignedSyncError::Storage(error.to_string()))?;
    let result = conn.execute(
        "INSERT INTO sync_signed_replay_reservations
         (space_id, message_id, origin_node_id, key_epoch, reserved_at_ms)
        VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            &verified.space_id,
            &verified.message_id,
            &verified.origin_node_id,
            key_epoch,
            reserved_at_ms,
        ],
    );
    match result {
        Ok(1) => match apply(conn, &verified) {
            Ok(value) => match conn.execute_batch(&format!("RELEASE SAVEPOINT {SAVEPOINT}")) {
                Ok(()) => Ok(value),
                Err(error) => {
                    rollback(conn);
                    Err(SignedSyncError::Storage(error.to_string()))
                }
            },
            Err(error) => {
                rollback(conn);
                Err(SignedSyncError::Apply(error))
            }
        },
        Ok(_) => {
            rollback(conn);
            Err(SignedSyncError::Replay)
        }
        Err(error) => {
            rollback(conn);
            if is_constraint(&error) {
                Err(SignedSyncError::Replay)
            } else {
                Err(SignedSyncError::Storage(error.to_string()))
            }
        }
    }
}

fn require_text(value: &str, field: &'static str) -> Result<(), SignedSyncError> {
    if value.trim().is_empty() {
        return Err(SignedSyncError::EmptyField { field });
    }
    Ok(())
}

fn is_constraint(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(code, _)
        if code.code == rusqlite::ffi::ErrorCode::ConstraintViolation
            && matches!(code.extended_code,
                rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
                    | rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE))
}

fn rollback(conn: &Connection) {
    let _ = conn.execute_batch(&format!(
        "ROLLBACK TO SAVEPOINT {SAVEPOINT}; RELEASE SAVEPOINT {SAVEPOINT}"
    ));
}
