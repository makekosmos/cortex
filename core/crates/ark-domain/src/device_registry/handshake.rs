use super::{encoding::encode_hex, DeviceId, RegistryEventError};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MAX_SESSION_ID_BYTES: usize = 128;
const MAX_SPACE_ID_BYTES: usize = 128;
const MAX_CAPABILITIES: usize = 32;
const MAX_CAPABILITY_ID_BYTES: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryHandshakeTranscriptDraft {
    pub session_id: String,
    pub space_id: String,
    pub initiator_device_id: DeviceId,
    pub responder_device_id: DeviceId,
    pub initiator_nonce: String,
    pub responder_nonce: String,
    pub protocol_version: u32,
    pub capabilities: Vec<String>,
}

/// Canonical identity proof input shared by the initiator and responder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryHandshakeTranscript {
    session_id: String,
    space_id: String,
    initiator_device_id: DeviceId,
    responder_device_id: DeviceId,
    initiator_nonce: String,
    responder_nonce: String,
    protocol_version: u32,
    capabilities: Vec<String>,
}

impl RegistryHandshakeTranscript {
    pub fn new(draft: RegistryHandshakeTranscriptDraft) -> Result<Self, RegistryEventError> {
        let RegistryHandshakeTranscriptDraft {
            session_id,
            space_id,
            initiator_device_id,
            responder_device_id,
            initiator_nonce,
            responder_nonce,
            protocol_version,
            capabilities,
        } = draft;
        let capabilities = capabilities.into_iter().collect::<BTreeSet<_>>();
        if session_id.is_empty()
            || session_id.len() > MAX_SESSION_ID_BYTES
            || session_id.chars().any(char::is_control)
        {
            return Err(RegistryEventError::InvalidField {
                field: "session_id",
            });
        }
        if space_id.is_empty()
            || space_id.len() > MAX_SPACE_ID_BYTES
            || space_id.chars().any(char::is_control)
        {
            return Err(RegistryEventError::InvalidField { field: "space_id" });
        }
        if initiator_device_id == responder_device_id {
            return Err(RegistryEventError::InvalidField {
                field: "handshake_devices",
            });
        }
        if !is_nonce(&initiator_nonce) || !is_nonce(&responder_nonce) {
            return Err(RegistryEventError::InvalidField { field: "nonce" });
        }
        if protocol_version == 0
            || capabilities.is_empty()
            || capabilities.len() > MAX_CAPABILITIES
            || capabilities.iter().any(|capability| {
                capability.is_empty()
                    || capability.len() > MAX_CAPABILITY_ID_BYTES
                    || !capability.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            })
        {
            return Err(RegistryEventError::InvalidField {
                field: "negotiated_protocol",
            });
        }
        Ok(Self {
            session_id,
            space_id,
            initiator_device_id,
            responder_device_id,
            initiator_nonce,
            responder_nonce,
            protocol_version,
            capabilities: capabilities.into_iter().collect(),
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn initiator_device_id(&self) -> &DeviceId {
        &self.initiator_device_id
    }

    pub fn responder_device_id(&self) -> &DeviceId {
        &self.responder_device_id
    }

    pub fn sign(
        &self,
        signer_device_id: DeviceId,
        signing_key: &SigningKey,
    ) -> Result<RegistryHandshakeProof, RegistryEventError> {
        if signer_device_id != self.initiator_device_id
            && signer_device_id != self.responder_device_id
        {
            return Err(RegistryEventError::ActorMismatch);
        }
        let signature = signing_key.sign(&self.signing_bytes()?);
        Ok(RegistryHandshakeProof {
            signer_device_id,
            signature: encode_hex(&signature.to_bytes()),
        })
    }

    fn signing_bytes(&self) -> Result<Vec<u8>, RegistryEventError> {
        let mut bytes = b"cortex-device-registry-handshake-v1\0".to_vec();
        bytes.extend(
            serde_json::to_vec(self).map_err(|_| RegistryEventError::InvalidField {
                field: "handshake_transcript",
            })?,
        );
        Ok(bytes)
    }
}

/// Proof that one enrolled device owns the key for this handshake transcript.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryHandshakeProof {
    pub(crate) signer_device_id: DeviceId,
    pub(crate) signature: String,
}

impl RegistryHandshakeProof {
    pub fn from_wire(
        signer_device_id: impl Into<String>,
        signature: impl Into<String>,
    ) -> Result<Self, RegistryEventError> {
        let signer_device_id = DeviceId::parse(signer_device_id)?;
        let signature = signature.into();
        decode_signature(&signature)?;
        Ok(Self {
            signer_device_id,
            signature,
        })
    }

    pub fn signer_device_id(&self) -> &DeviceId {
        &self.signer_device_id
    }

    pub fn signature(&self) -> &str {
        &self.signature
    }

    pub fn verify(
        &self,
        transcript: &RegistryHandshakeTranscript,
        expected_signer: &DeviceId,
        verifying_key: &VerifyingKey,
    ) -> Result<(), RegistryEventError> {
        if &self.signer_device_id != expected_signer
            || (expected_signer != &transcript.initiator_device_id
                && expected_signer != &transcript.responder_device_id)
        {
            return Err(RegistryEventError::ActorMismatch);
        }
        let signature_bytes = decode_signature(&self.signature)?;
        verifying_key
            .verify_strict(&transcript.signing_bytes()?, &signature_bytes)
            .map_err(|_| RegistryEventError::InvalidSignature)
    }
}

fn decode_signature(value: &str) -> Result<ed25519_dalek::Signature, RegistryEventError> {
    if value.len() != 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RegistryEventError::InvalidField { field: "signature" });
    }
    let mut bytes = [0_u8; 64];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        *byte = u8::from_str_radix(&value[start..start + 2], 16)
            .map_err(|_| RegistryEventError::InvalidField { field: "signature" })?;
    }
    Ok(ed25519_dalek::Signature::from_bytes(&bytes))
}

fn is_nonce(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn transcript() -> RegistryHandshakeTranscript {
        RegistryHandshakeTranscript::new(RegistryHandshakeTranscriptDraft {
            session_id: "session-1".into(),
            space_id: "space-1".into(),
            initiator_device_id: DeviceId::parse("device-a").unwrap(),
            responder_device_id: DeviceId::parse("device-b").unwrap(),
            initiator_nonce: "01".repeat(32),
            responder_nonce: "02".repeat(32),
            protocol_version: 1,
            capabilities: vec!["registry_history_v1".into()],
        })
        .unwrap()
    }

    #[test]
    fn proof_binds_both_devices_nonces_space_and_negotiated_capabilities() {
        let signer = SigningKey::from_bytes(&[17; 32]);
        let device_id = DeviceId::parse("device-a").unwrap();
        let original = transcript();
        let proof = original.sign(device_id.clone(), &signer).unwrap();
        proof
            .verify(&original, &device_id, &signer.verifying_key())
            .unwrap();

        let changed = RegistryHandshakeTranscript::new(RegistryHandshakeTranscriptDraft {
            session_id: "session-1".into(),
            space_id: "space-other".into(),
            initiator_device_id: DeviceId::parse("device-a").unwrap(),
            responder_device_id: DeviceId::parse("device-b").unwrap(),
            initiator_nonce: "01".repeat(32),
            responder_nonce: "02".repeat(32),
            protocol_version: 1,
            capabilities: vec!["registry_history_v1".into()],
        })
        .unwrap();
        assert_eq!(
            proof.verify(&changed, &device_id, &signer.verifying_key()),
            Err(RegistryEventError::InvalidSignature)
        );
    }

    #[test]
    fn proof_cannot_claim_a_different_signer() {
        let signer = SigningKey::from_bytes(&[18; 32]);
        let proof = transcript()
            .sign(DeviceId::parse("device-a").unwrap(), &signer)
            .unwrap();
        assert_eq!(
            proof.verify(
                &transcript(),
                &DeviceId::parse("device-b").unwrap(),
                &signer.verifying_key()
            ),
            Err(RegistryEventError::ActorMismatch)
        );
    }
}
