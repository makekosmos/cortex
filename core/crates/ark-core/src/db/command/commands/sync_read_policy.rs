use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};
use crate::types::VersionVector;
use ark_application::sync_storage::SyncReadClass;
use serde::Serialize;

#[derive(Debug, Clone)]
pub(crate) struct LoadPolicyBoundEntitiesPageQuery {
    vector: VersionVector,
    device_id: String,
    peer_device_id: String,
    class: CommandClass,
    offset: usize,
    limit: usize,
    expected_identity: ark_application::sync_storage::PolicyIdentity,
}

impl LoadPolicyBoundEntitiesPageQuery {
    pub(crate) fn new(
        vector: VersionVector,
        device_id: String,
        peer_device_id: String,
        class: SyncReadClass,
        offset: usize,
        limit: usize,
        expected_identity: ark_application::sync_storage::PolicyIdentity,
    ) -> Self {
        Self {
            vector,
            device_id,
            peer_device_id,
            class: class.into(),
            offset,
            limit,
            expected_identity,
        }
    }
}

impl sealed::Sealed for LoadPolicyBoundEntitiesPageQuery {}

impl StoreReadQuery for LoadPolicyBoundEntitiesPageQuery {
    type Output = ark_application::sync_storage::PolicyBoundSnapshotPage;

    fn class(&self) -> CommandClass {
        self.class
    }

    fn weight(&self) -> usize {
        serde_json::to_vec(&self.vector).map_or(usize::MAX, |bytes| {
            bytes.len()
                + self.device_id.len()
                + self.peer_device_id.len()
                + std::mem::size_of::<(usize, usize)>()
        }) + self.expected_identity.digest.len()
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        capabilities
            .sync()
            .policy_bound_entities_page(
                &self.peer_device_id,
                &self.expected_identity,
                &self.vector,
                &self.device_id,
                self.offset,
                self.limit,
            )
            .map_err(map_policy_bound_read_error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PolicyBoundFeedDeliveryPageQuery {
    peer_device_id: String,
    expected_identity: ark_application::sync_storage::PolicyIdentity,
    after_position: i64,
    high_watermark: i64,
    limit: usize,
}

impl PolicyBoundFeedDeliveryPageQuery {
    pub(crate) fn new(
        peer_device_id: String,
        expected_identity: ark_application::sync_storage::PolicyIdentity,
        after_position: i64,
        high_watermark: i64,
        limit: usize,
    ) -> Self {
        Self {
            peer_device_id,
            expected_identity,
            after_position,
            high_watermark,
            limit,
        }
    }
}

impl sealed::Sealed for PolicyBoundFeedDeliveryPageQuery {}

impl StoreReadQuery for PolicyBoundFeedDeliveryPageQuery {
    type Output = ark_application::sync_storage::PolicyBoundFeedPage;

    fn class(&self) -> CommandClass {
        CommandClass::SyncLive
    }

    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.peer_device_id.len()
            + self.expected_identity.digest.len()
    }

    fn read(
        self,
        context: &CommandContext,
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        capabilities
            .sync()
            .policy_bound_feed_page(
                &self.peer_device_id,
                &self.expected_identity,
                self.after_position,
                self.high_watermark,
                self.limit,
                crate::db::MAX_FEED_DELIVERY_BYTES,
            )
            .map_err(map_policy_bound_read_error)
    }
}

fn map_policy_bound_read_error(
    error: ark_application::sync_storage::PolicyBoundReadError,
) -> DbError {
    match error {
        ark_application::sync_storage::PolicyBoundReadError::StalePolicy => {
            DbError::StaleSyncPolicy
        }
        ark_application::sync_storage::PolicyBoundReadError::Storage(error) => {
            DbError::Storage(error)
        }
    }
}
