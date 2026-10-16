use super::super::{sealed, CommandClass, CommandContext, DbError, DbQuery};
use crate::application::sync_storage::CommittedFeedDeliveryPage;
use crate::application::sync_storage::SyncReadClass;
use crate::data_platform::SelectiveSyncProfile;
use crate::db::{
    committed_feed_snapshot, get_sync_kv, read_committed_feed_delivery_page,
    read_committed_feed_page, usage_log_complete_through, CommittedFeedEntry, FeedSnapshot,
    SqliteStorageBackend,
};
use crate::types::{SyncEntity, VersionVector};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(crate) struct LoadEntitiesQuery {
    vector: VersionVector,
    device_id: String,
    profile: Option<SelectiveSyncProfile>,
    peer_device_id: Option<String>,
    page: Option<(usize, usize)>,
    class: CommandClass,
}

impl LoadEntitiesQuery {
    pub(super) fn class(&self) -> CommandClass {
        self.class
    }

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
            peer_device_id: None,
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
            peer_device_id: None,
            page: Some((offset, limit)),
            class: class.into(),
        }
    }

    pub(crate) fn page_for_peer(
        vector: VersionVector,
        device_id: String,
        peer_device_id: String,
        class: SyncReadClass,
        offset: usize,
        limit: usize,
    ) -> Self {
        Self {
            vector,
            device_id,
            profile: None,
            peer_device_id: Some(peer_device_id),
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
        self.read_with_policy_identity(context, connection)
            .map(|(_, entities)| entities)
    }
}

impl LoadEntitiesQuery {
    pub(super) fn read_with_policy_identity(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<
        (
            Option<crate::application::sync_storage::PolicyIdentity>,
            Vec<SyncEntity>,
        ),
        DbError,
    > {
        context.require_sync_peer_or_system()?;
        let Self {
            vector,
            device_id,
            mut profile,
            peer_device_id,
            page,
            class: _,
        } = self;
        let mut policy_identity = None;
        if let Some(peer_device_id) = peer_device_id {
            context.require_sync_peer_matches(&peer_device_id)?;
            let policy =
                crate::data_platform::load_accepted_peer_sync_policy(connection, &peer_device_id)
                    .map_err(DbError::from_application_error)?;
            policy_identity = Some(policy.as_ref().map_or_else(
                crate::data_platform::default_sync_policy_identity,
                |policy| policy.identity(),
            ));
            profile = policy.and_then(|policy| policy.active.then(|| policy.selective_profile()));
        }
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
        let entities = match (page, profile) {
            (None, Some(profile)) => {
                crate::data_platform::filter_entities_for_profile(&profile, &entities)
            }
            _ => entities,
        };
        Ok((policy_identity, entities))
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
pub(crate) struct CommittedFeedSnapshotQuery;

impl sealed::Sealed for CommittedFeedSnapshotQuery {}

impl DbQuery for CommittedFeedSnapshotQuery {
    type Output = FeedSnapshot;

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        1
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_or_system()?;
        committed_feed_snapshot(connection).map_err(DbError::Storage)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CommittedFeedPageQuery {
    after_position: i64,
    high_watermark: i64,
    limit: usize,
}

impl CommittedFeedPageQuery {
    pub(crate) fn new(after_position: i64, high_watermark: i64, limit: usize) -> Self {
        Self {
            after_position,
            high_watermark,
            limit,
        }
    }
}

impl sealed::Sealed for CommittedFeedPageQuery {}

impl DbQuery for CommittedFeedPageQuery {
    type Output = Vec<CommittedFeedEntry>;

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
        read_committed_feed_page(
            connection,
            self.after_position,
            self.high_watermark,
            self.limit,
        )
        .map_err(DbError::Storage)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CommittedFeedDeliveryPageQuery {
    after_position: i64,
    high_watermark: i64,
    limit: usize,
}

impl CommittedFeedDeliveryPageQuery {
    pub(crate) fn new(after_position: i64, high_watermark: i64, limit: usize) -> Self {
        Self {
            after_position,
            high_watermark,
            limit,
        }
    }
}

impl sealed::Sealed for CommittedFeedDeliveryPageQuery {}

impl DbQuery for CommittedFeedDeliveryPageQuery {
    type Output = CommittedFeedDeliveryPage;

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
        read_committed_feed_delivery_page(
            connection,
            self.after_position,
            self.high_watermark,
            self.limit,
        )
        .map_err(DbError::Storage)
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
