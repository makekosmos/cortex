#[cfg(debug_assertions)]
use super::Connection;
use super::{SqliteWriter, StoreTransactionFailure, WriteCapabilities};
use ark_domain::device_registry::{
    state::DeviceRegistry, DeviceId, DeviceRecord, RegistryCommand, RegistryEvent,
    RegistryEventDraft, RegistryHandshakeProof, RegistryHandshakeTranscript, RegistryHead,
    RegistryOperation, SigningPublicKey,
};
use rusqlite::Transaction;

const IROH_SECRET_KEY_KV_KEY: &str = "iroh.secret_key";

#[derive(Debug)]
pub enum RegistryLocalMutationError {
    Invalid(String),
    Conflict(String),
    Permission(String),
    Storage(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEnrollmentOutcome {
    Applied {
        event_id: String,
        event_hash: String,
    },
    AlreadyApplied {
        event_id: Option<String>,
    },
}

pub struct RegistryLocalWrites<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

impl WriteCapabilities<'_, '_> {
    pub fn registry_local_mutation(&mut self) -> RegistryLocalWrites<'_, '_> {
        RegistryLocalWrites {
            transaction: self.transaction,
        }
    }
}

impl RegistryLocalWrites<'_, '_> {
    pub fn sign_handshake(
        &mut self,
        signer_device_id: &str,
        transcript: &RegistryHandshakeTranscript,
        candidate_secret_key_hex: &str,
    ) -> Result<RegistryHandshakeProof, RegistryLocalMutationError> {
        let signer = DeviceId::parse(signer_device_id).map_err(registry_error)?;
        let secret = load_or_insert_secret(self.transaction, candidate_secret_key_hex)?;
        let local_key = signing_public_key(&secret)?;
        let registry = crate::device_registry::load_registry(self.transaction)
            .map_err(RegistryLocalMutationError::Storage)?;
        let device = registry.device(&signer).ok_or_else(|| {
            RegistryLocalMutationError::Permission("device_registry_peer_unknown".into())
        })?;
        if device.signing_public_key != local_key {
            return Err(RegistryLocalMutationError::Conflict(
                "device_registry_local_identity_conflict".into(),
            ));
        }
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret.to_bytes());
        transcript
            .sign(signer, &signing_key)
            .map_err(registry_error)
    }

    pub fn append_event(
        &mut self,
        actor_device_id: &str,
        command: RegistryCommand,
        candidate_secret_key_hex: &str,
    ) -> Result<Option<RegistryEvent>, RegistryLocalMutationError> {
        let transaction = self.transaction;
        let actor_id = DeviceId::parse(actor_device_id).map_err(registry_error)?;
        let secret = load_or_insert_secret(transaction, candidate_secret_key_hex)?;
        let local_key = signing_public_key(&secret)?;
        let mut registry = crate::device_registry::load_registry(transaction)
            .map_err(RegistryLocalMutationError::Storage)?;
        if registry.device(&actor_id).is_none() {
            let record = device_record(actor_id.clone(), local_key.clone(), 1, 1);
            crate::device_registry::upsert_device_projection(transaction, &record)
                .map_err(RegistryLocalMutationError::Storage)?;
            registry.add_trust_anchor(record);
        }
        let actor = registry.device(&actor_id).ok_or_else(|| {
            RegistryLocalMutationError::Storage("device_registry_local_identity_missing".into())
        })?;
        if actor.signing_public_key != local_key {
            return Err(RegistryLocalMutationError::Conflict(
                "device_registry_local_identity_conflict".into(),
            ));
        }
        if actor.disabled_event_id.is_some() {
            return Err(RegistryLocalMutationError::Permission(
                "device_registry_local_device_disabled".into(),
            ));
        }

        let (target_device_id, operation) = match command {
            RegistryCommand::Rename {
                target_device_id,
                display_name,
            } => {
                let target = registry.device(&target_device_id).ok_or_else(|| {
                    RegistryLocalMutationError::Invalid("device_registry_target_missing".into())
                })?;
                if target.display_name.as_ref() == Some(&display_name) {
                    return Ok(None);
                }
                (target_device_id, RegistryOperation::Rename { display_name })
            }
            RegistryCommand::Disable { target_device_id } => {
                if target_device_id == actor_id {
                    return Err(RegistryLocalMutationError::Invalid(
                        "device_registry_self_disable".into(),
                    ));
                }
                let target = registry.device(&target_device_id).ok_or_else(|| {
                    RegistryLocalMutationError::Invalid("device_registry_target_missing".into())
                })?;
                if target.disabled_event_id.is_some() {
                    return Ok(None);
                }
                (target_device_id, RegistryOperation::Disable)
            }
        };

        let (actor_sequence, previous_event_hash) = match registry.head(&actor_id) {
            Some(head) => (
                head.actor_sequence.checked_add(1).ok_or_else(|| {
                    RegistryLocalMutationError::Conflict(
                        "device_registry_sequence_exhausted".into(),
                    )
                })?,
                Some(head.event_hash.clone()),
            ),
            None => (1, None),
        };
        let mut hlc = ark_domain::hlc::HLC::now(actor_id.as_str());
        hlc.counter = actor_sequence.saturating_sub(1);
        let event = RegistryEvent::sign(
            RegistryEventDraft {
                actor_device_id: actor_id,
                actor_sequence,
                previous_event_hash,
                target_device_id,
                actor_key_epoch: actor.key_epoch,
                actor_admission_epoch: actor.admission_epoch,
                hlc: hlc.to_string(),
                operation,
            },
            &secret,
        )
        .map_err(registry_error)?;
        persist_signed_event(transaction, &mut registry, &event)?;
        Ok(Some(event))
    }

    pub fn enroll_paired_device(
        &mut self,
        local_device_id: &str,
        target_device_id: &str,
        target_signing_key: &SigningPublicKey,
        transport_public_key: &str,
        candidate_secret_key_hex: &str,
    ) -> Result<DeviceEnrollmentOutcome, RegistryLocalMutationError> {
        let transaction = self.transaction;
        let local_device = DeviceId::parse(local_device_id).map_err(registry_error)?;
        let target_device = DeviceId::parse(target_device_id).map_err(registry_error)?;
        if local_device == target_device {
            return Err(RegistryLocalMutationError::Invalid(
                "device_registry_self_enrollment".into(),
            ));
        }
        let secret = load_or_insert_secret(transaction, candidate_secret_key_hex)?;
        let local_key = signing_public_key(&secret)?;
        let local_identity =
            crate::device_registry::load_identity(transaction, local_device.as_str())
                .map_err(RegistryLocalMutationError::Storage)?;
        if let Some(identity) = &local_identity {
            if identity.signing_public_key.as_slice() != local_key.as_bytes() {
                return Err(RegistryLocalMutationError::Conflict(
                    "device_registry_local_identity_conflict".into(),
                ));
            }
            if identity.disabled_event_id.is_some() {
                return Err(RegistryLocalMutationError::Permission(
                    "device_registry_local_device_disabled".into(),
                ));
            }
        } else {
            let record = device_record(local_device.clone(), local_key.clone(), 1, 1);
            crate::device_registry::upsert_device_projection(transaction, &record)
                .map_err(RegistryLocalMutationError::Storage)?;
        }

        if let Some(identity) =
            crate::device_registry::load_identity(transaction, target_device.as_str())
                .map_err(RegistryLocalMutationError::Storage)?
        {
            if identity.signing_public_key.as_slice() != target_signing_key.as_bytes() {
                return Err(RegistryLocalMutationError::Conflict(
                    "device_registry_signing_key_conflict".into(),
                ));
            }
            if identity.disabled_event_id.is_some() {
                return Err(RegistryLocalMutationError::Permission(
                    "device_registry_device_disabled".into(),
                ));
            }
            persist_transport_pin(transaction, target_device.as_str(), transport_public_key)?;
            return Ok(DeviceEnrollmentOutcome::AlreadyApplied {
                event_id: identity.updated_event_id,
            });
        }

        let previous_head =
            crate::device_registry::load_actor_head(transaction, local_device.as_str())
                .map_err(RegistryLocalMutationError::Storage)?;
        let (actor_key_epoch, actor_admission_epoch) = local_identity
            .as_ref()
            .map(|identity| (identity.key_epoch, identity.admission_epoch))
            .unwrap_or((1, 1));
        let sequence = previous_head
            .as_ref()
            .map(|(sequence, _)| sequence.saturating_add(1))
            .unwrap_or(1);
        let previous_event_hash = previous_head.map(|(_, hash)| hash);
        let mut registry = DeviceRegistry::default();
        registry.add_trust_anchor(device_record(
            local_device.clone(),
            local_key,
            actor_key_epoch,
            actor_admission_epoch,
        ));
        if let Some(previous_event_hash) = previous_event_hash.as_ref() {
            registry.set_head(RegistryHead {
                actor_device_id: local_device.clone(),
                actor_sequence: sequence - 1,
                event_hash: previous_event_hash.clone(),
            });
        }
        let mut hlc = ark_domain::hlc::HLC::now(local_device.as_str());
        hlc.counter = sequence.saturating_sub(1);
        let event = RegistryEvent::sign(
            RegistryEventDraft {
                actor_device_id: local_device,
                actor_sequence: sequence,
                previous_event_hash,
                target_device_id: target_device.clone(),
                actor_key_epoch,
                actor_admission_epoch,
                hlc: hlc.to_string(),
                operation: RegistryOperation::Enroll {
                    signing_public_key: target_signing_key.clone(),
                    key_epoch: 1,
                    display_name: None,
                },
            },
            &secret,
        )
        .map_err(registry_error)?;
        let event_hash = persist_signed_event(transaction, &mut registry, &event)?;
        persist_transport_pin(transaction, target_device.as_str(), transport_public_key)?;
        Ok(DeviceEnrollmentOutcome::Applied {
            event_id: event.event_id().to_owned(),
            event_hash,
        })
    }
}

fn persist_signed_event(
    transaction: &Transaction<'_>,
    registry: &mut DeviceRegistry,
    event: &RegistryEvent,
) -> Result<String, RegistryLocalMutationError> {
    let outcome = registry
        .apply(event.clone())
        .map_err(|error| RegistryLocalMutationError::Conflict(error.to_string()))?;
    let ark_domain::device_registry::RegistryApplyOutcome::Applied { event_hash } = outcome;
    let projection = registry.device(event.target_device_id()).ok_or_else(|| {
        RegistryLocalMutationError::Storage(
            "device_registry_projection_missing_after_apply".to_owned(),
        )
    })?;
    i64::try_from(event.actor_sequence()).map_err(|_| {
        RegistryLocalMutationError::Conflict("device_registry_sequence_exhausted".into())
    })?;
    let event_json = serde_json::to_string(event)
        .map_err(|error| RegistryLocalMutationError::Storage(error.to_string()))?;
    crate::device_registry::persist_event_and_projection(
        transaction,
        event,
        &event_hash,
        &event_json,
        projection,
    )
    .map_err(RegistryLocalMutationError::Storage)?;
    Ok(event_hash)
}

fn load_or_insert_secret(
    transaction: &Transaction<'_>,
    candidate_hex: &str,
) -> Result<ed25519_dalek::SigningKey, RegistryLocalMutationError> {
    let persisted =
        crate::sync_kv::get_or_insert_sync_kv(transaction, IROH_SECRET_KEY_KV_KEY, candidate_hex)
            .map_err(RegistryLocalMutationError::Storage)?;
    let bytes = decode_secret_key_hex(&persisted).ok_or_else(|| {
        RegistryLocalMutationError::Storage(
            "iroh transport: stored secret key is not valid hex".into(),
        )
    })?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&bytes))
}

fn decode_secret_key_hex(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let high = (value.as_bytes()[index * 2] as char).to_digit(16)?;
        let low = (value.as_bytes()[index * 2 + 1] as char).to_digit(16)?;
        *byte = ((high << 4) | low) as u8;
    }
    Some(bytes)
}

fn signing_public_key(
    secret: &ed25519_dalek::SigningKey,
) -> Result<SigningPublicKey, RegistryLocalMutationError> {
    SigningPublicKey::from_bytes(secret.verifying_key().as_bytes().to_owned())
        .map_err(registry_error)
}

fn device_record(
    device_id: DeviceId,
    key: SigningPublicKey,
    key_epoch: u64,
    admission_epoch: u64,
) -> DeviceRecord {
    DeviceRecord {
        device_id,
        signing_public_key: key,
        key_epoch,
        admission_epoch,
        display_name: None,
        disabled_event_id: None,
        display_name_hlc: None,
        updated_hlc: None,
        updated_event_id: None,
    }
}

fn persist_transport_pin(
    transaction: &Transaction<'_>,
    device_id: &str,
    transport_public_key: &str,
) -> Result<(), RegistryLocalMutationError> {
    crate::device_registry::persist_transport_key_pin(transaction, device_id, transport_public_key)
        .map_err(|error| match error {
            crate::device_registry::TransportKeyPinError::Conflict => {
                RegistryLocalMutationError::Conflict(
                    "device_registry_transport_key_conflict".into(),
                )
            }
            crate::device_registry::TransportKeyPinError::Storage(error) => {
                RegistryLocalMutationError::Storage(error)
            }
        })
}

fn registry_error(
    error: ark_domain::device_registry::RegistryEventError,
) -> RegistryLocalMutationError {
    RegistryLocalMutationError::Invalid(error.to_string())
}

impl SqliteWriter {
    #[cfg(debug_assertions)]
    pub fn sign_registry_handshake_on_connection(
        connection: &mut Connection,
        signer_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
        candidate_secret_key_hex: &str,
    ) -> Result<
        ark_domain::device_registry::RegistryHandshakeProof,
        StoreTransactionFailure<RegistryLocalMutationError>,
    > {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            capabilities.registry_local_mutation().sign_handshake(
                signer_device_id,
                transcript,
                candidate_secret_key_hex,
            )
        })
    }

    pub fn sign_registry_handshake(
        &mut self,
        signer_device_id: &str,
        transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
        candidate_secret_key_hex: &str,
    ) -> Result<
        ark_domain::device_registry::RegistryHandshakeProof,
        StoreTransactionFailure<RegistryLocalMutationError>,
    > {
        self.write_with_capabilities(|capabilities| {
            capabilities.registry_local_mutation().sign_handshake(
                signer_device_id,
                transcript,
                candidate_secret_key_hex,
            )
        })
    }

    pub fn append_local_registry_event(
        &mut self,
        actor_device_id: &str,
        command: RegistryCommand,
        candidate_secret_key_hex: &str,
    ) -> Result<Option<RegistryEvent>, StoreTransactionFailure<RegistryLocalMutationError>> {
        self.write_with_capabilities(|capabilities| {
            capabilities.registry_local_mutation().append_event(
                actor_device_id,
                command,
                candidate_secret_key_hex,
            )
        })
    }

    pub fn enroll_paired_device(
        &mut self,
        local_device_id: &str,
        target_device_id: &str,
        target_signing_key: &SigningPublicKey,
        transport_public_key: &str,
        candidate_secret_key_hex: &str,
    ) -> Result<DeviceEnrollmentOutcome, StoreTransactionFailure<RegistryLocalMutationError>> {
        self.write_with_capabilities(|capabilities| {
            capabilities.registry_local_mutation().enroll_paired_device(
                local_device_id,
                target_device_id,
                target_signing_key,
                transport_public_key,
                candidate_secret_key_hex,
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn append_local_registry_event_on_connection(
        connection: &mut Connection,
        actor_device_id: &str,
        command: RegistryCommand,
        candidate_secret_key_hex: &str,
    ) -> Result<Option<RegistryEvent>, StoreTransactionFailure<RegistryLocalMutationError>> {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            capabilities.registry_local_mutation().append_event(
                actor_device_id,
                command,
                candidate_secret_key_hex,
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn enroll_paired_device_on_connection(
        connection: &mut Connection,
        local_device_id: &str,
        target_device_id: &str,
        target_signing_key: &SigningPublicKey,
        transport_public_key: &str,
        candidate_secret_key_hex: &str,
    ) -> Result<DeviceEnrollmentOutcome, StoreTransactionFailure<RegistryLocalMutationError>> {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            capabilities.registry_local_mutation().enroll_paired_device(
                local_device_id,
                target_device_id,
                target_signing_key,
                transport_public_key,
                candidate_secret_key_hex,
            )
        })
    }
}
