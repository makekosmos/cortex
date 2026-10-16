use super::super::{
    sealed, CommandClass, CommandContext, DbCommand, DbError, DbQuery,
    DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY,
};
use crate::domain::device_registry::{
    state::DeviceRegistry, DeviceId, DeviceRecord, DisplayName, RegistryEvent, RegistryHead,
    SigningPublicKey,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use sha2::Digest;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceRegistryEntryView {
    pub(crate) device_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceRegistrySnapshot {
    pub(crate) local_device_id: String,
    pub(crate) devices: Vec<DeviceRegistryEntryView>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LoadDeviceRegistryQuery;

impl sealed::Sealed for LoadDeviceRegistryQuery {}

impl DbQuery for LoadDeviceRegistryQuery {
    type Output = DeviceRegistrySnapshot;

    fn class(&self) -> CommandClass {
        CommandClass::Interactive
    }

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let local_device_id =
            crate::db::get_sync_kv(connection, DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY)
                .map_err(DbError::Storage)?
                .ok_or_else(|| DbError::Invalid("device_registry_local_identity_missing".into()))?;
        let mut statement = connection
            .prepare(
                "SELECT device_id, display_name, disabled_event_id IS NULL
                 FROM device_registry_projection ORDER BY device_id",
            )
            .map_err(storage_error)?;
        let mut devices = statement
            .query_map([], |row| {
                Ok(DeviceRegistryEntryView {
                    device_id: row.get(0)?,
                    display_name: row.get(1)?,
                    enabled: row.get(2)?,
                })
            })
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        if !devices
            .iter()
            .any(|device| device.device_id == local_device_id)
        {
            devices.push(DeviceRegistryEntryView {
                device_id: local_device_id.clone(),
                display_name: None,
                enabled: true,
            });
            devices.sort_by(|left, right| left.device_id.cmp(&right.device_id));
        }
        Ok(DeviceRegistrySnapshot {
            local_device_id,
            devices,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LoadRegistryEventsQuery {
    after_position: i64,
    high_watermark: Option<i64>,
    limit: usize,
}

impl LoadRegistryEventsQuery {
    pub(crate) fn new(after_position: i64, high_watermark: Option<i64>, limit: usize) -> Self {
        Self {
            after_position,
            high_watermark,
            limit,
        }
    }
}

impl sealed::Sealed for LoadRegistryEventsQuery {}

impl DbQuery for LoadRegistryEventsQuery {
    type Output = (i64, usize, Vec<RegistryEvent>);

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_or_system()?;
        if self.after_position < 0 || self.limit == 0 || self.limit > 64 {
            return Err(DbError::Invalid("device_registry_page_bounds".into()));
        }
        let high_watermark = match self.high_watermark {
            Some(high_watermark) if high_watermark >= self.after_position => high_watermark,
            Some(_) => return Err(DbError::Invalid("device_registry_page_cursor".into())),
            None => connection
                .query_row(
                    "SELECT COALESCE(MAX(registry_position), 0) FROM device_registry_events",
                    [],
                    |row| row.get(0),
                )
                .map_err(storage_error)?,
        };
        let event_count = connection
            .query_row(
                "SELECT COUNT(*) FROM device_registry_events WHERE registry_position <= ?1",
                [high_watermark],
                |row| row.get::<_, i64>(0),
            )
            .map_err(storage_error)?;
        let event_count = usize::try_from(event_count)
            .map_err(|_| DbError::Storage("negative_device_registry_event_count".into()))?;
        let mut statement = connection
            .prepare(
                "SELECT event_json FROM device_registry_events
                 WHERE registry_position > ?1 AND registry_position <= ?2
                 ORDER BY registry_position LIMIT ?3",
            )
            .map_err(storage_error)?;
        let events = statement
            .query_map(
                params![self.after_position, high_watermark, self.limit as i64],
                |row| row.get::<_, String>(0),
            )
            .map_err(storage_error)?
            .map(|result| {
                let raw = result.map_err(storage_error)?;
                serde_json::from_str(&raw)
                    .map_err(|error| DbError::Storage(format!("invalid registry event: {error}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((high_watermark, event_count, events))
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApplyRegistryEventsCommand {
    sender_device_id: String,
    events: Vec<RegistryEvent>,
}

impl ApplyRegistryEventsCommand {
    pub(crate) fn new(sender_device_id: String, events: Vec<RegistryEvent>) -> Self {
        Self {
            sender_device_id,
            events,
        }
    }
}

impl sealed::Sealed for ApplyRegistryEventsCommand {}

impl DbCommand for ApplyRegistryEventsCommand {
    type Output = crate::replication_storage::RegistryPageApply;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.sender_device_id)?;
        if self.events.len() > 32 {
            return Err(DbError::Invalid("device_registry_page_bounds".into()));
        }
        // A disabled device may reconnect to learn the signed revocation and
        // remaining registry history. Registry control is safe here because
        // ordinary data still requires the separate admitted-session gate.
        super::device_registry_admission::require_registry_peer_known(
            transaction,
            &self.sender_device_id,
        )?;
        let applied_events = apply_events(transaction, &self.events)?;
        let disabled_device_ids = applied_events
            .iter()
            .filter(|event| {
                matches!(
                    event.operation(),
                    crate::domain::device_registry::RegistryOperation::Disable
                )
            })
            .map(|event| event.target_device_id().as_str().to_owned())
            .collect();
        let sender_is_admitted = transaction
            .query_row(
                "SELECT disabled_event_id IS NULL FROM device_registry_projection
                 WHERE device_id = ?1",
                [&self.sender_device_id],
                |row| row.get::<_, bool>(0),
            )
            .optional()
            .map_err(storage_error)?
            .unwrap_or(false);
        let local_device_id =
            crate::db::get_sync_kv(transaction, DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY)
                .map_err(DbError::Storage)?;
        let local_device_is_admitted = local_device_id
            .as_deref()
            .map(|device_id| {
                super::device_registry_admission::registry_device_is_admitted(
                    transaction,
                    device_id,
                )
            })
            .transpose()?
            .flatten()
            .unwrap_or(true);
        Ok(crate::replication_storage::RegistryPageApply {
            digest: registry_digest(transaction)?,
            sender_is_admitted,
            local_device_is_admitted,
            disabled_device_ids,
            applied_events,
        })
    }
}

pub(super) fn require_registry_peer_admitted(
    transaction: &Transaction<'_>,
    device_id: &str,
) -> Result<(), DbError> {
    let admitted = transaction
        .query_row(
            "SELECT disabled_event_id IS NULL FROM device_registry_projection
             WHERE device_id = ?1",
            [device_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()
        .map_err(storage_error)?
        .unwrap_or(false);
    if admitted {
        Ok(())
    } else {
        Err(DbError::Permission("sync_peer_not_admitted".into()))
    }
}

pub(super) fn apply_events(
    transaction: &Transaction<'_>,
    events: &[RegistryEvent],
) -> Result<Vec<RegistryEvent>, DbError> {
    let mut registry = load_registry(transaction)?;
    let mut applied_events = Vec::new();
    for event in events {
        let event_json =
            serde_json::to_string(event).map_err(|error| DbError::Storage(error.to_string()))?;
        let existing = transaction
            .query_row(
                "SELECT event_json FROM device_registry_events
                 WHERE actor_device_id = ?1 AND actor_sequence = ?2",
                params![event.actor_device_id().as_str(), event.actor_sequence()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(storage_error)?;
        if let Some(existing) = existing {
            if existing == event_json {
                continue;
            }
            return Err(DbError::Conflict(
                "device_registry_sequence_conflict".into(),
            ));
        }

        let outcome = registry
            .apply(event.clone())
            .map_err(|error| DbError::Permission(error.to_string()))?;
        let crate::domain::device_registry::RegistryApplyOutcome::Applied { event_hash } = outcome;
        let target = registry
            .device(event.target_device_id())
            .ok_or_else(|| DbError::Storage("device_registry_target_missing_after_apply".into()))?;
        let position: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(registry_position), 0) + 1 FROM device_registry_events",
                [],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        let sequence = i64::try_from(event.actor_sequence())
            .map_err(|_| DbError::Conflict("device_registry_sequence_exhausted".into()))?;
        transaction
            .execute(
                "INSERT INTO device_registry_events (
                    registry_position, actor_device_id, actor_sequence, event_id,
                    target_device_id, previous_event_hash, event_hash, event_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    position,
                    event.actor_device_id().as_str(),
                    sequence,
                    event.event_id(),
                    event.target_device_id().as_str(),
                    event.previous_event_hash(),
                    event_hash,
                    event_json,
                ],
            )
            .map_err(storage_error)?;
        persist_projection(transaction, target)?;
        applied_events.push(event.clone());
    }
    Ok(applied_events)
}

pub(super) fn load_registry(transaction: &Connection) -> Result<DeviceRegistry, DbError> {
    let mut registry = DeviceRegistry::default();
    let mut statement = transaction
        .prepare(
            "SELECT device_id, signing_public_key, key_epoch, admission_epoch,
                    display_name, disabled_event_id, display_name_hlc, updated_hlc,
                    updated_event_id
             FROM device_registry_projection ORDER BY device_id",
        )
        .map_err(storage_error)?;
    let devices = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, u64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    drop(statement);
    for (id, key, key_epoch, admission_epoch, name, disabled, name_hlc, updated_hlc, event_id) in
        devices
    {
        let key: [u8; 32] = key
            .try_into()
            .map_err(|_| DbError::Storage("invalid_device_registry_key_length".into()))?;
        registry.add_trust_anchor(DeviceRecord {
            device_id: DeviceId::parse(id).map_err(registry_error)?,
            signing_public_key: SigningPublicKey::from_bytes(key).map_err(registry_error)?,
            key_epoch,
            admission_epoch,
            display_name: name
                .map(DisplayName::parse)
                .transpose()
                .map_err(registry_error)?,
            disabled_event_id: disabled,
            display_name_hlc: name_hlc,
            updated_hlc,
            updated_event_id: event_id,
        });
    }

    let mut statement = transaction
        .prepare(
            "SELECT actor_device_id, actor_sequence, event_hash
             FROM device_registry_events AS event
             WHERE actor_sequence = (
                 SELECT MAX(actor_sequence) FROM device_registry_events
                 WHERE actor_device_id = event.actor_device_id
             ) ORDER BY actor_device_id",
        )
        .map_err(storage_error)?;
    let heads = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    for (actor, sequence, event_hash) in heads {
        registry.set_head(RegistryHead {
            actor_device_id: DeviceId::parse(actor).map_err(registry_error)?,
            actor_sequence: sequence,
            event_hash,
        });
    }
    Ok(registry)
}

fn persist_projection(transaction: &Transaction<'_>, device: &DeviceRecord) -> Result<(), DbError> {
    transaction
        .execute(
            "INSERT INTO device_registry_projection (
                device_id, signing_public_key, key_epoch, admission_epoch, display_name,
                disabled_event_id, display_name_hlc, updated_hlc, updated_event_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(device_id) DO UPDATE SET
                signing_public_key = excluded.signing_public_key,
                key_epoch = excluded.key_epoch,
                admission_epoch = excluded.admission_epoch,
                display_name = excluded.display_name,
                disabled_event_id = excluded.disabled_event_id,
                display_name_hlc = excluded.display_name_hlc,
                updated_hlc = excluded.updated_hlc,
                updated_event_id = excluded.updated_event_id",
            params![
                device.device_id.as_str(),
                device.signing_public_key.as_bytes().as_slice(),
                device.key_epoch,
                device.admission_epoch,
                device.display_name.as_ref().map(DisplayName::as_str),
                device.disabled_event_id,
                device.display_name_hlc,
                device.updated_hlc,
                device.updated_event_id,
            ],
        )
        .map_err(storage_error)?;
    Ok(())
}

fn registry_digest(transaction: &Transaction<'_>) -> Result<String, DbError> {
    let mut statement = transaction
        .prepare(
            "SELECT actor_device_id, actor_sequence, event_hash
             FROM device_registry_events ORDER BY actor_device_id, actor_sequence",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    let bytes = serde_json::to_vec(&rows).map_err(|error| DbError::Storage(error.to_string()))?;
    let digest = sha2::Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn registry_error(error: crate::domain::device_registry::RegistryEventError) -> DbError {
    DbError::Invalid(error.to_string())
}

fn storage_error(error: rusqlite::Error) -> DbError {
    DbError::Storage(error.to_string())
}
