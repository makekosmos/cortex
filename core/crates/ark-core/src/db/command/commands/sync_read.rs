use super::super::{sealed, CommandClass, CommandContext, DbError, DbQuery};
use crate::application::sync_storage::SyncReadClass;
use crate::data_platform::SelectiveSyncProfile;
use crate::db::{get_sync_kv, usage_log_complete_through, SqliteStorageBackend};
use crate::types::{SyncEntity, VersionVector};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct LoadEntitiesQuery {
    vector: VersionVector,
    device_id: String,
    profile: Option<SelectiveSyncProfile>,
    page: Option<(usize, usize)>,
    class: CommandClass,
}

impl LoadEntitiesQuery {
    pub(crate) fn all(
        vector: VersionVector,
        device_id: String,
        profile: Option<SelectiveSyncProfile>,
        class: SyncReadClass,
    ) -> Self {
        Self {
            vector,
            device_id,
            profile,
            page: None,
            class: class.into(),
        }
    }

    pub(crate) fn page(
        vector: VersionVector,
        device_id: String,
        profile: Option<SelectiveSyncProfile>,
        class: SyncReadClass,
        offset: usize,
        limit: usize,
    ) -> Self {
        Self {
            vector,
            device_id,
            profile,
            page: Some((offset, limit)),
            class: class.into(),
        }
    }
}

impl sealed::Sealed for LoadEntitiesQuery {}

impl DbQuery for LoadEntitiesQuery {
    type Output = Vec<SyncEntity>;

    fn class(&self) -> CommandClass {
        self.class
    }

    fn weight(&self) -> usize {
        serde_json::to_vec(&self.vector).map_or(usize::MAX, |bytes| {
            bytes.len() + self.device_id.len() + std::mem::size_of::<(usize, usize)>()
        })
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_or_system()?;
        let Self {
            vector,
            device_id,
            profile,
            page,
            class: _,
        } = self;
        let entities = match page {
            None => {
                SqliteStorageBackend::collect_entities_blocking(connection, &vector, &device_id)
            }
            Some((offset, limit)) => {
                if let Some(profile) = &profile {
                    let all = SqliteStorageBackend::collect_entities_blocking(
                        connection, &vector, &device_id,
                    );
                    crate::data_platform::filter_entities_for_profile(profile, &all)
                        .into_iter()
                        .skip(offset)
                        .take(limit)
                        .collect()
                } else {
                    SqliteStorageBackend::collect_entities_page_blocking(
                        connection, &vector, &device_id, offset, limit,
                    )
                }
            }
        };
        Ok(match (page, profile) {
            (None, Some(profile)) => {
                crate::data_platform::filter_entities_for_profile(&profile, &entities)
            }
            _ => entities,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UsageCompleteThroughQuery {
    class: CommandClass,
}

impl UsageCompleteThroughQuery {
    pub(crate) fn new(class: SyncReadClass) -> Self {
        Self {
            class: class.into(),
        }
    }
}

impl sealed::Sealed for UsageCompleteThroughQuery {}

impl DbQuery for UsageCompleteThroughQuery {
    type Output = HashMap<String, u64>;

    fn class(&self) -> CommandClass {
        self.class
    }

    fn weight(&self) -> usize {
        1
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        if context.class != self.class {
            return Err(DbError::Invalid("sync query class mismatch".into()));
        }
        context.require_sync_peer_or_system()?;
        Ok(usage_log_complete_through(connection, None).unwrap_or_default())
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GetSyncKeyValueQuery {
    key: String,
}

impl GetSyncKeyValueQuery {
    pub(crate) fn new(key: String) -> Self {
        Self { key }
    }
}

impl sealed::Sealed for GetSyncKeyValueQuery {}

impl DbQuery for GetSyncKeyValueQuery {
    type Output = Option<String>;

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        get_sync_kv(connection, &self.key).map_err(DbError::Storage)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AuthorizedTransportKeyQuery {
    device_id: String,
}

impl AuthorizedTransportKeyQuery {
    pub(crate) fn new(device_id: String) -> Self {
        Self { device_id }
    }
}

impl sealed::Sealed for AuthorizedTransportKeyQuery {}

impl DbQuery for AuthorizedTransportKeyQuery {
    type Output = Option<String>;

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        if context.authenticated_peer_id()? != self.device_id {
            return Err(DbError::Permission("sync peer principal mismatch".into()));
        }
        Ok(crate::db::load_authorized_node(connection, &self.device_id)
            .map_err(DbError::Storage)?
            .and_then(|node| node.transport_public_key))
    }
}

impl From<SyncReadClass> for CommandClass {
    fn from(class: SyncReadClass) -> Self {
        match class {
            SyncReadClass::SyncLive => Self::SyncLive,
            SyncReadClass::Backfill => Self::Backfill,
        }
    }
}
