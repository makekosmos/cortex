use super::super::{sealed, CommandClass, CommandContext, DbError, DbQuery};
use rusqlite::{Connection, OptionalExtension, Transaction};
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

impl DbQuery for RegistryPeerAdmissionQuery {
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
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        let peer_admitted =
            registry_device_is_admitted(connection, &self.peer_device_id)?.unwrap_or(false);
        let local_device_id =
            crate::db::get_sync_kv(connection, crate::db::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY)
                .map_err(DbError::Storage)?;
        let local_admitted = local_device_id
            .as_deref()
            .map(|device_id| registry_device_is_admitted(connection, device_id))
            .transpose()?
            .flatten()
            .unwrap_or(true);
        Ok(peer_admitted && local_admitted)
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

impl DbQuery for RegistryDeviceEnabledQuery {
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
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        registry_device_is_admitted(connection, &self.peer_device_id)
    }
}

pub(super) fn require_registry_peer_known(
    transaction: &Transaction<'_>,
    device_id: &str,
) -> Result<(), DbError> {
    let known = transaction
        .query_row(
            "SELECT 1 FROM device_registry_projection WHERE device_id = ?1",
            [device_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(storage_error)?
        .is_some();
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
    connection
        .query_row(
            "SELECT disabled_event_id IS NULL FROM device_registry_projection
             WHERE device_id = ?1",
            [device_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage_error)
}

fn storage_error(error: rusqlite::Error) -> DbError {
    DbError::from_sqlite(error)
}
