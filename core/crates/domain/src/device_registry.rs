mod encoding;
mod handshake;
pub mod state;
mod validation;

pub use handshake::{
    RegistryHandshakeProof, RegistryHandshakeTranscript, RegistryHandshakeTranscriptDraft,
};
pub use state::{DeviceRecord, RegistryApplyError, RegistryApplyOutcome, RegistryHead};

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use encoding::{canonical_bytes, decode_signature, encode_hex};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use validation::{validate_draft, validate_event};

const EVENT_SCHEMA_VERSION: u8 = 1;
const MAX_DEVICE_ID_BYTES: usize = 128;
const MAX_DISPLAY_NAME_BYTES: usize = 128;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryEventError {
    #[error("{field} is invalid")]
    InvalidField { field: &'static str },
    #[error("registry event signature is invalid")]
    InvalidSignature,
    #[error("registry event actor does not match the enrolled device")]
    ActorMismatch,
    #[error("registry event key epoch is stale or unexpected")]
    KeyEpochMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct DeviceId(String);

impl TryFrom<String> for DeviceId {
    type Error = RegistryEventError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl DeviceId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RegistryEventError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_DEVICE_ID_BYTES
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(RegistryEventError::InvalidField { field: "device_id" });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct DisplayName(String);

impl TryFrom<String> for DisplayName {
    type Error = RegistryEventError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl DisplayName {
    pub fn parse(value: impl Into<String>) -> Result<Self, RegistryEventError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_DISPLAY_NAME_BYTES
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(RegistryEventError::InvalidField {
                field: "display_name",
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "[u8; 32]")]
pub struct SigningPublicKey([u8; 32]);

impl TryFrom<[u8; 32]> for SigningPublicKey {
    type Error = RegistryEventError;

    fn try_from(bytes: [u8; 32]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl SigningPublicKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, RegistryEventError> {
        let key =
            VerifyingKey::from_bytes(&bytes).map_err(|_| RegistryEventError::InvalidField {
                field: "public_key",
            })?;
        if key.is_weak() {
            return Err(RegistryEventError::InvalidField {
                field: "public_key",
            });
        }
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn verifying_key(&self) -> Result<VerifyingKey, RegistryEventError> {
        VerifyingKey::from_bytes(&self.0).map_err(|_| RegistryEventError::InvalidField {
            field: "public_key",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegistryOperation {
    Enroll {
        signing_public_key: SigningPublicKey,
        key_epoch: u64,
        display_name: Option<DisplayName>,
    },
    Rename {
        display_name: DisplayName,
    },
    Disable,
    RebindKey {
        signing_public_key: SigningPublicKey,
        key_epoch: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegistryCommand {
    Rename {
        target_device_id: DeviceId,
        display_name: DisplayName,
    },
    Disable {
        target_device_id: DeviceId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEventDraft {
    pub actor_device_id: DeviceId,
    pub actor_sequence: u64,
    pub previous_event_hash: Option<String>,
    pub target_device_id: DeviceId,
    pub actor_key_epoch: u64,
    pub actor_admission_epoch: u64,
    pub hlc: String,
    pub operation: RegistryOperation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryEvent {
    schema_version: u8,
    event_id: String,
    actor_device_id: DeviceId,
    actor_sequence: u64,
    previous_event_hash: Option<String>,
    target_device_id: DeviceId,
    actor_key_epoch: u64,
    actor_admission_epoch: u64,
    hlc: String,
    operation: RegistryOperation,
    signature: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedRegistryEvent(RegistryEvent);

impl VerifiedRegistryEvent {
    pub fn event(&self) -> &RegistryEvent {
        &self.0
    }

    pub fn event_hash(&self) -> Result<String, RegistryEventError> {
        let mut material = self.0.canonical_signing_bytes()?;
        material.extend_from_slice(&decode_signature(&self.0.signature)?.to_bytes());
        Ok(encode_hex(&Sha256::digest(material)))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SigningPayload<'a> {
    schema_version: u8,
    event_id: &'a str,
    actor_device_id: &'a DeviceId,
    actor_sequence: u64,
    previous_event_hash: &'a Option<String>,
    target_device_id: &'a DeviceId,
    actor_key_epoch: u64,
    actor_admission_epoch: u64,
    hlc: &'a str,
    operation: &'a RegistryOperation,
}

impl RegistryEvent {
    pub fn sign(
        draft: RegistryEventDraft,
        signing_key: &SigningKey,
    ) -> Result<Self, RegistryEventError> {
        validate_draft(&draft)?;
        let event_id = format!(
            "{}:{}",
            draft.actor_device_id.as_str(),
            draft.actor_sequence
        );
        let payload = SigningPayload {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id: &event_id,
            actor_device_id: &draft.actor_device_id,
            actor_sequence: draft.actor_sequence,
            previous_event_hash: &draft.previous_event_hash,
            target_device_id: &draft.target_device_id,
            actor_key_epoch: draft.actor_key_epoch,
            actor_admission_epoch: draft.actor_admission_epoch,
            hlc: &draft.hlc,
            operation: &draft.operation,
        };
        let bytes = canonical_bytes(&payload)?;
        let signature = signing_key.sign(&bytes);
        Ok(Self {
            schema_version: EVENT_SCHEMA_VERSION,
            event_id,
            actor_device_id: draft.actor_device_id,
            actor_sequence: draft.actor_sequence,
            previous_event_hash: draft.previous_event_hash,
            target_device_id: draft.target_device_id,
            actor_key_epoch: draft.actor_key_epoch,
            actor_admission_epoch: draft.actor_admission_epoch,
            hlc: draft.hlc,
            operation: draft.operation,
            signature: encode_hex(&signature.to_bytes()),
        })
    }

    pub fn verify(
        &self,
        expected_actor: &DeviceId,
        expected_key_epoch: u64,
        public_key: &VerifyingKey,
    ) -> Result<VerifiedRegistryEvent, RegistryEventError> {
        if &self.actor_device_id != expected_actor {
            return Err(RegistryEventError::ActorMismatch);
        }
        if self.actor_key_epoch != expected_key_epoch {
            return Err(RegistryEventError::KeyEpochMismatch);
        }
        if public_key.is_weak() {
            return Err(RegistryEventError::InvalidField {
                field: "public_key",
            });
        }
        validate_event(self)?;
        let signature = decode_signature(&self.signature)?;
        public_key
            .verify_strict(&self.canonical_signing_bytes()?, &signature)
            .map_err(|_| RegistryEventError::InvalidSignature)?;
        Ok(VerifiedRegistryEvent(self.clone()))
    }

    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>, RegistryEventError> {
        validate_event(self)?;
        canonical_bytes(&SigningPayload {
            schema_version: self.schema_version,
            event_id: &self.event_id,
            actor_device_id: &self.actor_device_id,
            actor_sequence: self.actor_sequence,
            previous_event_hash: &self.previous_event_hash,
            target_device_id: &self.target_device_id,
            actor_key_epoch: self.actor_key_epoch,
            actor_admission_epoch: self.actor_admission_epoch,
            hlc: &self.hlc,
            operation: &self.operation,
        })
    }

    pub fn actor_device_id(&self) -> &DeviceId {
        &self.actor_device_id
    }

    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn previous_event_hash(&self) -> Option<&str> {
        self.previous_event_hash.as_deref()
    }

    pub fn actor_sequence(&self) -> u64 {
        self.actor_sequence
    }

    pub fn actor_key_epoch(&self) -> u64 {
        self.actor_key_epoch
    }

    pub fn actor_admission_epoch(&self) -> u64 {
        self.actor_admission_epoch
    }

    pub fn hlc(&self) -> &str {
        &self.hlc
    }

    pub fn target_device_id(&self) -> &DeviceId {
        &self.target_device_id
    }

    pub fn operation(&self) -> &RegistryOperation {
        &self.operation
    }
}

#[cfg(test)]
#[path = "device_registry/tests.rs"]
mod tests;
