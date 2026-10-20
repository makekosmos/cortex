use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};
use rusqlite::{Connection, Transaction};
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

pub(super) fn require_registry_peer_known(
    transaction: &Transaction<'_>,
    device_id: &str,
) -> Result<(), DbError> {
    let known = ark_store::device_registry::is_device_known(transaction, device_id)
        .map_err(DbError::Storage)?;
    if known {
        Ok(())
    } else {
        Err(DbError::Permission("sync_peer_not_registered".into()))
    }
}

pub(super) fn registry_device_is_admitted(
    connection: &Connection,
    device_id: &str,
) -> Result<Option<bool>, DbError> {
    ark_store::device_registry::is_device_admitted(connection, device_id).map_err(DbError::Storage)
}
