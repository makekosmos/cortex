use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::domain::device_registry::{
    state::DeviceRegistry, DeviceId, DeviceRecord, RegistryEvent, RegistryEventDraft, RegistryHead,
    RegistryOperation, SigningPublicKey,
};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeviceEnrollmentOutcome {
    Applied {
        event_id: String,
        event_hash: String,
    },
    AlreadyApplied {
        event_id: Option<String>,
    },
}

#[derive(Debug, Serialize)]
pub(crate) struct EnrollPairedDeviceCommand {
    local_device_id: String,
    target_device_id: String,
    transport_public_key: String,
}

impl EnrollPairedDeviceCommand {
    pub(crate) fn new(
        local_device_id: String,
        target_device_id: String,
        transport_public_key: String,
    ) -> Self {
        Self {
            local_device_id,
            target_device_id,
            transport_public_key,
        }
    }
}

impl sealed::Sealed for EnrollPairedDeviceCommand {}

impl DbCommand for EnrollPairedDeviceCommand {
    type Output = DeviceEnrollmentOutcome;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        enroll_paired_device_in_transaction(
            transaction,
            &self.local_device_id,
            &self.target_device_id,
            &self.transport_public_key,
        )
    }
}

fn enroll_paired_device_in_transaction(
    transaction: &Transaction<'_>,
    local_device_id: &str,
    target_device_id: &str,
    transport_public_key: &str,
) -> Result<DeviceEnrollmentOutcome, DbError> {
    let local_device = DeviceId::parse(local_device_id).map_err(registry_error)?;
    let target_device = DeviceId::parse(target_device_id).map_err(registry_error)?;
    if local_device == target_device {
        return Err(DbError::Invalid("device_registry_self_enrollment".into()));
    }
    let endpoint_id: iroh::EndpointId = transport_public_key
        .parse()
        .map_err(|_| DbError::Invalid("device_registry_invalid_endpoint_id".into()))?;
    let target_key =
        SigningPublicKey::from_bytes(*endpoint_id.as_bytes()).map_err(registry_error)?;
    let secret = crate::iroh_transport::load_or_generate_secret_key(transaction)
        .map_err(DbError::Storage)?;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret.to_bytes());
    let local_key =
        SigningPublicKey::from_bytes(*secret.public().as_bytes()).map_err(registry_error)?;
    let local_identity = transaction
        .query_row(
            "SELECT signing_public_key, disabled_event_id, key_epoch, admission_epoch
             FROM device_registry_projection WHERE device_id = ?1",
            [local_device.as_str()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, u64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(storage_error)?;
    if let Some((stored_key, disabled_event_id, _, _)) = &local_identity {
        if stored_key.as_slice() != local_key.as_bytes() {
            return Err(DbError::Conflict(
                "device_registry_local_identity_conflict".into(),
            ));
        }
        if disabled_event_id.is_some() {
            return Err(DbError::Permission(
                "device_registry_local_device_disabled".into(),
            ));
        }
    } else {
        transaction
            .execute(
                "INSERT INTO device_registry_projection (
                    device_id, signing_public_key, key_epoch, admission_epoch,
                    display_name, disabled_event_id, display_name_hlc, updated_hlc,
                    updated_event_id
                 ) VALUES (?1, ?2, 1, 1, NULL, NULL, NULL, NULL, NULL)",
                params![local_device.as_str(), local_key.as_bytes().as_slice()],
            )
            .map_err(storage_error)?;
    }

    let existing = transaction
        .query_row(
            "SELECT signing_public_key, disabled_event_id, updated_event_id
             FROM device_registry_projection WHERE device_id = ?1",
            [target_device.as_str()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(storage_error)?;
    if let Some((stored_key, disabled_event_id, event_id)) = existing {
        if stored_key.as_slice() != target_key.as_bytes() {
            return Err(DbError::Conflict(
                "device_registry_signing_key_conflict".into(),
            ));
        }
        if disabled_event_id.is_some() {
            return Err(DbError::Permission(
                "device_registry_device_disabled".into(),
            ));
        }
        persist_transport_pin(transaction, target_device.as_str(), transport_public_key)?;
        return Ok(DeviceEnrollmentOutcome::AlreadyApplied { event_id });
    }

    let previous_head = transaction
        .query_row(
            "SELECT actor_sequence, event_hash FROM device_registry_events
             WHERE actor_device_id = ?1 ORDER BY actor_sequence DESC LIMIT 1",
            [local_device.as_str()],
            |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(storage_error)?;
    let (actor_key_epoch, actor_admission_epoch) = local_identity
        .as_ref()
        .map(|(_, _, key_epoch, admission_epoch)| (*key_epoch, *admission_epoch))
        .unwrap_or((1, 1));
    let sequence = previous_head
        .as_ref()
        .map(|(sequence, _)| sequence.saturating_add(1))
        .unwrap_or(1);
    let previous_event_hash = previous_head.map(|(_, hash)| hash);
    let mut registry = DeviceRegistry::default();
    registry.add_trust_anchor(DeviceRecord {
        device_id: local_device.clone(),
        signing_public_key: local_key.clone(),
        key_epoch: actor_key_epoch,
        admission_epoch: actor_admission_epoch,
        display_name: None,
        disabled_event_id: None,
        display_name_hlc: None,
        updated_hlc: None,
        updated_event_id: None,
    });
    if let Some(previous_event_hash) = previous_event_hash.as_ref() {
        registry.set_head(RegistryHead {
            actor_device_id: local_device.clone(),
            actor_sequence: sequence - 1,
            event_hash: previous_event_hash.clone(),
        });
    }
    let mut hlc = crate::hlc::HLC::now(local_device.as_str());
    hlc.counter = sequence.saturating_sub(1);
    let event = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: local_device.clone(),
            actor_sequence: sequence,
            previous_event_hash,
            target_device_id: target_device.clone(),
            actor_key_epoch,
            actor_admission_epoch,
            hlc: hlc.to_string(),
            operation: RegistryOperation::Enroll {
                signing_public_key: target_key.clone(),
                key_epoch: 1,
                display_name: None,
            },
        },
        &signing_key,
    )
    .map_err(registry_error)?;
    let outcome = registry
        .apply(event.clone())
        .map_err(registry_apply_error)?;
    let crate::domain::device_registry::RegistryApplyOutcome::Applied { event_hash } = outcome;
    let target_projection = registry
        .device(&target_device)
        .ok_or_else(|| DbError::Storage("device_registry_projection_missing_after_apply".into()))?;
    let event_json =
        serde_json::to_string(&event).map_err(|error| DbError::Storage(error.to_string()))?;
    let previous_event_hash = event.previous_event_hash();
    let sequence_i64 = i64::try_from(sequence)
        .map_err(|_| DbError::Conflict("device_registry_sequence_exhausted".into()))?;
    let registry_position = next_registry_position(transaction)?;
    transaction
        .execute(
            "INSERT INTO device_registry_events (
                registry_position, actor_device_id, actor_sequence, event_id, target_device_id,
                previous_event_hash, event_hash, event_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                registry_position,
                local_device.as_str(),
                sequence_i64,
                event.event_id(),
                target_device.as_str(),
                previous_event_hash,
                event_hash,
                event_json,
            ],
        )
        .map_err(storage_error)?;
    transaction
        .execute(
            "INSERT INTO device_registry_projection (
                device_id, signing_public_key, key_epoch, admission_epoch,
                display_name, disabled_event_id, display_name_hlc, updated_hlc,
                updated_event_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8)",
            params![
                target_device.as_str(),
                target_projection.signing_public_key.as_bytes().as_slice(),
                target_projection.key_epoch,
                target_projection.admission_epoch,
                target_projection
                    .display_name
                    .as_ref()
                    .map(|name| name.as_str()),
                target_projection.display_name_hlc.as_deref(),
                target_projection.updated_hlc.as_deref(),
                target_projection.updated_event_id.as_deref(),
            ],
        )
        .map_err(storage_error)?;
    persist_transport_pin(transaction, target_device.as_str(), transport_public_key)?;

    Ok(DeviceEnrollmentOutcome::Applied {
        event_id: event.event_id().to_owned(),
        event_hash,
    })
}

fn next_registry_position(transaction: &Transaction<'_>) -> Result<i64, DbError> {
    transaction
        .query_row(
            "SELECT COALESCE(MAX(registry_position), 0) + 1 FROM device_registry_events",
            [],
            |row| row.get(0),
        )
        .map_err(storage_error)
}

fn persist_transport_pin(
    transaction: &Transaction<'_>,
    device_id: &str,
    transport_public_key: &str,
) -> Result<(), DbError> {
    let raw = crate::db::get_sync_kv(
        transaction,
        crate::application::sync_storage::PEER_TRANSPORT_KEYS_KEY,
    )
    .map_err(|error| DbError::Storage(error.to_string()))?;
    let mut keys: HashMap<String, String> = raw
        .map(|raw| serde_json::from_str(&raw))
        .transpose()
        .map_err(|error| DbError::Storage(error.to_string()))?
        .unwrap_or_default();
    if keys
        .get(device_id)
        .is_some_and(|existing| existing != transport_public_key)
    {
        return Err(DbError::Conflict(
            "device_registry_transport_key_conflict".into(),
        ));
    }
    keys.insert(device_id.to_owned(), transport_public_key.to_owned());
    let encoded =
        serde_json::to_string(&keys).map_err(|error| DbError::Storage(error.to_string()))?;
    transaction
        .execute(
            "INSERT INTO sync_kv(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![
                crate::application::sync_storage::PEER_TRANSPORT_KEYS_KEY,
                encoded
            ],
        )
        .map_err(storage_error)?;
    Ok(())
}

fn registry_error(error: crate::domain::device_registry::RegistryEventError) -> DbError {
    DbError::Invalid(error.to_string())
}

fn registry_apply_error(error: crate::domain::device_registry::RegistryApplyError) -> DbError {
    DbError::Conflict(error.to_string())
}

fn storage_error(error: rusqlite::Error) -> DbError {
    DbError::Storage(error.to_string())
}
