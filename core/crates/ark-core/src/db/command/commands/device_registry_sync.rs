use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};
use ark_domain::device_registry::RegistryEvent;
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
    pub(crate) sender_device_id: String,
    pub(crate) events: Vec<RegistryEvent>,
}

impl ApplyRegistryEventsCommand {
    pub(crate) fn new(sender_device_id: String, events: Vec<RegistryEvent>) -> Self {
        Self {
            sender_device_id,
            events,
        }
    }
}

impl ApplyRegistryEventsCommand {
    pub(crate) fn weight(&self) -> usize {
        super::serialized_weight(self)
    }
}
