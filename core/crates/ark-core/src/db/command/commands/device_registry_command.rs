use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::domain::device_registry::{
    DeviceId, DeviceRecord, RegistryCommand, RegistryEvent, RegistryEventDraft, RegistryOperation,
    SigningPublicKey,
};
use rusqlite::{params, Transaction};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AppendLocalRegistryEventCommand {
    actor_device_id: String,
    command: RegistryCommand,
}

impl AppendLocalRegistryEventCommand {
    pub(crate) fn new(actor_device_id: String, command: RegistryCommand) -> Self {
        Self {
            actor_device_id,
            command,
        }
    }
}

impl sealed::Sealed for AppendLocalRegistryEventCommand {}

impl DbCommand for AppendLocalRegistryEventCommand {
    type Output = Option<RegistryEvent>;

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
        append_local_registry_event(transaction, &self.actor_device_id, self.command)
    }
}

fn append_local_registry_event(
    transaction: &Transaction<'_>,
    actor_device_id: &str,
    command: RegistryCommand,
) -> Result<Option<RegistryEvent>, DbError> {
    let actor_id = DeviceId::parse(actor_device_id).map_err(registry_error)?;
    let mut registry = super::device_registry_sync::load_registry(transaction)?;
    let secret = crate::iroh_transport::load_or_generate_secret_key(transaction)
        .map_err(DbError::Storage)?;
    let local_key =
        SigningPublicKey::from_bytes(*secret.public().as_bytes()).map_err(registry_error)?;
    if registry.device(&actor_id).is_none() {
        transaction
            .execute(
                "INSERT INTO device_registry_projection (
                    device_id, signing_public_key, key_epoch, admission_epoch,
                    display_name, disabled_event_id, display_name_hlc, updated_hlc,
                    updated_event_id
                 ) VALUES (?1, ?2, 1, 1, NULL, NULL, NULL, NULL, NULL)",
                params![actor_id.as_str(), local_key.as_bytes().as_slice()],
            )
            .map_err(storage_error)?;
        registry.add_trust_anchor(DeviceRecord {
            device_id: actor_id.clone(),
            signing_public_key: local_key.clone(),
            key_epoch: 1,
            admission_epoch: 1,
            display_name: None,
            disabled_event_id: None,
            display_name_hlc: None,
            updated_hlc: None,
            updated_event_id: None,
        });
    }
    let actor = registry
        .device(&actor_id)
        .ok_or_else(|| DbError::Storage("device_registry_local_identity_missing".into()))?;
    if actor.signing_public_key != local_key {
        return Err(DbError::Conflict(
            "device_registry_local_identity_conflict".into(),
        ));
    }
    if actor.disabled_event_id.is_some() {
        return Err(DbError::Permission(
            "device_registry_local_device_disabled".into(),
        ));
    }

    let (target_device_id, operation) = match command {
        RegistryCommand::Rename {
            target_device_id,
            display_name,
        } => {
            let target = registry
                .device(&target_device_id)
                .ok_or_else(|| DbError::Invalid("device_registry_target_missing".into()))?;
            if target.display_name.as_ref() == Some(&display_name) {
                return Ok(None);
            }
            (target_device_id, RegistryOperation::Rename { display_name })
        }
        RegistryCommand::Disable { target_device_id } => {
            if target_device_id == actor_id {
                return Err(DbError::Invalid("device_registry_self_disable".into()));
            }
            let target = registry
                .device(&target_device_id)
                .ok_or_else(|| DbError::Invalid("device_registry_target_missing".into()))?;
            if target.disabled_event_id.is_some() {
                return Ok(None);
            }
            (target_device_id, RegistryOperation::Disable)
        }
    };

    let (actor_sequence, previous_event_hash) = match registry.head(&actor_id) {
        Some(head) => (
            head.actor_sequence
                .checked_add(1)
                .ok_or_else(|| DbError::Conflict("device_registry_sequence_exhausted".into()))?,
            Some(head.event_hash.clone()),
        ),
        None => (1, None),
    };
    let hlc = crate::hlc::HLC::now(actor_id.as_str()).to_string();
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret.to_bytes());
    let event = RegistryEvent::sign(
        RegistryEventDraft {
            actor_device_id: actor_id.clone(),
            actor_sequence,
            previous_event_hash,
            target_device_id,
            actor_key_epoch: actor.key_epoch,
            actor_admission_epoch: actor.admission_epoch,
            hlc,
            operation,
        },
        &signing_key,
    )
    .map_err(registry_error)?;
    let _ = super::device_registry_sync::apply_events(transaction, std::slice::from_ref(&event))?;
    Ok(Some(event))
}

fn registry_error(error: crate::domain::device_registry::RegistryEventError) -> DbError {
    DbError::Invalid(error.to_string())
}

fn storage_error(error: rusqlite::Error) -> DbError {
    DbError::from_sqlite(error)
}
