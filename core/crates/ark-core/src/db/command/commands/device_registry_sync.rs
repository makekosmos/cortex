use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError, StoreReadQuery};
use ark_domain::device_registry::RegistryEvent;
use ark_store::sync_kv::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY;
use rusqlite::Transaction;
use serde::Serialize;

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

impl StoreReadQuery for LoadDeviceRegistryQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let registry = capabilities.device_registry();
        let local_device_id = registry
            .local_device_id()
            .map_err(DbError::Storage)?
            .ok_or_else(|| DbError::Invalid("device_registry_local_identity_missing".into()))?;
        let mut devices = registry
            .entries()
            .map_err(DbError::Storage)?
            .into_iter()
            .map(|entry| DeviceRegistryEntryView {
                device_id: entry.device_id,
                display_name: entry.display_name,
                enabled: entry.enabled,
            })
            .collect::<Vec<_>>();
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

impl StoreReadQuery for LoadRegistryEventsQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_or_system()?;
        if self.after_position < 0
            || self.limit == 0
            || self.limit > ark_store::device_registry::MAX_REGISTRY_EVENTS_PAGE_SIZE
        {
            return Err(DbError::Invalid("device_registry_page_bounds".into()));
        }
        if self
            .high_watermark
            .is_some_and(|high_watermark| high_watermark < self.after_position)
        {
            return Err(DbError::Invalid("device_registry_page_cursor".into()));
        }
        capabilities
            .device_registry()
            .events_page(self.after_position, self.high_watermark, self.limit)
            .map_err(DbError::Storage)
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
                    ark_domain::device_registry::RegistryOperation::Disable
                )
            })
            .map(|event| event.target_device_id().as_str().to_owned())
            .collect();
        let sender_is_admitted =
            ark_store::device_registry::is_device_admitted(transaction, &self.sender_device_id)
                .map_err(DbError::Storage)?
                .unwrap_or(false);
        let local_device_id =
            ark_store::sync_kv::get_sync_kv(transaction, DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY)
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
            digest: ark_store::device_registry::registry_digest(transaction)
                .map_err(DbError::Storage)?,
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
    let admitted = ark_store::device_registry::is_device_admitted(transaction, device_id)
        .map_err(DbError::Storage)?
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
    let mut registry =
        ark_store::device_registry::load_registry(transaction).map_err(DbError::Storage)?;
    let mut applied_events = Vec::new();
    for event in events {
        let event_json =
            serde_json::to_string(event).map_err(|error| DbError::Storage(error.to_string()))?;
        let existing = ark_store::device_registry::event_json_for_actor_sequence(
            transaction,
            event.actor_device_id().as_str(),
            event.actor_sequence(),
        )
        .map_err(DbError::Storage)?;
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
        let ark_domain::device_registry::RegistryApplyOutcome::Applied { event_hash } = outcome;
        let target = registry
            .device(event.target_device_id())
            .ok_or_else(|| DbError::Storage("device_registry_target_missing_after_apply".into()))?;
        i64::try_from(event.actor_sequence())
            .map_err(|_| DbError::Conflict("device_registry_sequence_exhausted".into()))?;
        ark_store::device_registry::persist_event_and_projection(
            transaction,
            event,
            &event_hash,
            &event_json,
            target,
        )
        .map_err(DbError::Storage)?;
        applied_events.push(event.clone());
    }
    Ok(applied_events)
}
