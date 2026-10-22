use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};
use crate::types::{SyncEntity, VersionVector};
use ark_application::sync_storage::SyncReadClass;
use ark_application::sync_storage::{CommittedFeedDeliveryPage, CommittedFeedEntry, FeedSnapshot};
use ark_domain::selective_sync::SelectiveSyncProfile;
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

impl StoreReadQuery for LoadEntitiesQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        self.read_with_policy_identity(context, capabilities)
            .map(|(_, entities)| entities)
    }
}

impl LoadEntitiesQuery {
    pub(super) fn read_with_policy_identity(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<
        (
            Option<ark_domain::sync_policy::PolicyIdentity>,
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
            let policy = capabilities
                .sync()
                .accepted_peer_policy(&peer_device_id)
                .map_err(DbError::from_application_error)?;
            policy_identity = Some(policy.as_ref().map_or_else(
                ark_application::sync_policy::default_sync_policy_identity,
                |policy| policy.identity(),
            ));
            profile = policy.and_then(|policy| policy.active.then(|| policy.selective_profile()));
        }
        let entities = match page {
            None => capabilities
                .sync()
                .collect_entities(&vector, &device_id)
                .map_err(DbError::Storage)?,
            Some((offset, limit)) => {
                if let Some(profile) = &profile {
                    let all = capabilities
                        .sync()
                        .collect_entities(&vector, &device_id)
                        .map_err(DbError::Storage)?;
                    ark_domain::selective_sync::filter_entities_for_profile(profile, &all)
                        .into_iter()
                        .skip(offset)
                        .take(limit)
                        .collect()
                } else {
                    capabilities
                        .sync()
                        .collect_entities_page(&vector, &device_id, offset, limit)
                        .map_err(DbError::Storage)?
                }
            }
        };
        let entities = match (page, profile) {
            (None, Some(profile)) => {
                ark_domain::selective_sync::filter_entities_for_profile(&profile, &entities)
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

impl StoreReadQuery for UsageCompleteThroughQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        if context.class != self.class {
            return Err(DbError::Invalid("sync query class mismatch".into()));
        }
        context.require_sync_peer_or_system()?;
        Ok(capabilities.sync().usage_complete_through())
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CommittedFeedSnapshotQuery;

impl sealed::Sealed for CommittedFeedSnapshotQuery {}

impl StoreReadQuery for CommittedFeedSnapshotQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_or_system()?;
        capabilities
            .sync()
            .committed_feed_snapshot()
            .map_err(DbError::Storage)
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

impl StoreReadQuery for CommittedFeedPageQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_or_system()?;
        capabilities
            .sync()
            .committed_feed_page(self.after_position, self.high_watermark, self.limit)
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

impl StoreReadQuery for CommittedFeedDeliveryPageQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_or_system()?;
        capabilities
            .sync()
            .committed_feed_delivery_page(
                self.after_position,
                self.high_watermark,
                self.limit,
                crate::db::MAX_FEED_DELIVERY_BYTES,
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

impl StoreReadQuery for GetSyncKeyValueQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        capabilities
            .sync()
            .sync_key_value(&self.key)
            .map_err(DbError::Storage)
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

impl StoreReadQuery for AuthorizedTransportKeyQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        if context.authenticated_peer_id()? != self.device_id {
            return Err(DbError::Permission("sync peer principal mismatch".into()));
        }
        capabilities
            .sync()
            .authorized_transport_key(&self.device_id)
            .map_err(DbError::Storage)
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
