use super::super::{sealed, CommandClass, CommandContext, DbError, DbQuery};
use super::sync_read::LoadEntitiesQuery;
use crate::types::VersionVector;
use ark_application::sync_storage::SyncReadClass;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone)]
pub(crate) struct LoadPolicyBoundEntitiesPageQuery {
    query: LoadEntitiesQuery,
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
            query: LoadEntitiesQuery::page_for_peer(
                vector,
                device_id,
                peer_device_id,
                class,
                offset,
                limit,
            ),
            expected_identity,
        }
    }
}

impl sealed::Sealed for LoadPolicyBoundEntitiesPageQuery {}

impl DbQuery for LoadPolicyBoundEntitiesPageQuery {
    type Output = ark_application::sync_storage::PolicyBoundSnapshotPage;

    fn class(&self) -> CommandClass {
        self.query.class()
    }

    fn weight(&self) -> usize {
        self.query.weight()
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        connection
            .execute_batch("SAVEPOINT sync_policy_snapshot_page")
            .map_err(|error| DbError::Storage(error.to_string()))?;
        let result = self.query.read_with_policy_identity(context, connection);
        match result {
            Ok((Some(identity), entities)) if identity == self.expected_identity => {
                connection
                    .execute_batch("RELEASE sync_policy_snapshot_page")
                    .map_err(|error| DbError::Storage(error.to_string()))?;
                Ok(ark_application::sync_storage::PolicyBoundSnapshotPage { identity, entities })
            }
            Ok((Some(_), _)) => {
                connection
                    .execute_batch(
                        "ROLLBACK TO sync_policy_snapshot_page; RELEASE sync_policy_snapshot_page",
                    )
                    .map_err(|error| DbError::Storage(error.to_string()))?;
                Err(DbError::Permission(
                    "stale_or_unnegotiated_sync_policy".into(),
                ))
            }
            Ok((None, _)) => {
                connection
                    .execute_batch(
                        "ROLLBACK TO sync_policy_snapshot_page; RELEASE sync_policy_snapshot_page",
                    )
                    .map_err(|error| DbError::Storage(error.to_string()))?;
                Err(DbError::Storage(
                    "policy_bound_snapshot_identity_missing".into(),
                ))
            }
            Err(error) => {
                connection
                    .execute_batch(
                        "ROLLBACK TO sync_policy_snapshot_page; RELEASE sync_policy_snapshot_page",
                    )
                    .map_err(|rollback| DbError::Storage(rollback.to_string()))?;
                Err(error)
            }
        }
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

impl DbQuery for PolicyBoundFeedDeliveryPageQuery {
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
        connection: &Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        crate::db::read_committed_feed_delivery_page_for_peer(
            connection,
            &self.peer_device_id,
            &self.expected_identity,
            self.after_position,
            self.high_watermark,
            self.limit,
        )
        .map_err(DbError::Storage)
    }
}
