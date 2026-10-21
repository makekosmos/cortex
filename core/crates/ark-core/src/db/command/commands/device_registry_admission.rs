use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RegistryPeerAdmissionQuery {
    peer_device_id: String,
}

impl RegistryPeerAdmissionQuery {
    pub(crate) fn new(peer_device_id: String) -> Self {
        Self { peer_device_id }
    }
}

impl sealed::Sealed for RegistryPeerAdmissionQuery {}

impl StoreReadQuery for RegistryPeerAdmissionQuery {
    type Output = bool;

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        self.peer_device_id.len()
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        capabilities
            .sync()
            .sync_peer_is_admitted(&self.peer_device_id)
            .map_err(DbError::Storage)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct RegistryDeviceEnabledQuery {
    peer_device_id: String,
}

impl RegistryDeviceEnabledQuery {
    pub(crate) fn new(peer_device_id: String) -> Self {
        Self { peer_device_id }
    }
}

impl sealed::Sealed for RegistryDeviceEnabledQuery {}

impl StoreReadQuery for RegistryDeviceEnabledQuery {
    type Output = Option<bool>;

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        self.peer_device_id.len()
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        capabilities
            .sync()
            .device_is_admitted(&self.peer_device_id)
            .map_err(DbError::Storage)
    }
}
