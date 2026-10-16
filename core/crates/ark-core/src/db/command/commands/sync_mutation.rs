use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::application::sync_storage::{OutboundAck, OutboundProgress};
use crate::canonical_types::pending::ReplayNotice;
use crate::db::{ensure_usage_sequence_migrated, set_sync_kv, SqliteStorageBackend};
use crate::types::SyncEntity;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) enum CommittedFeedProgressOperation {
    BeginOutboundPolicy {
        peer_device_id: String,
        policy_generation: i64,
    },
    CompleteOutboundSnapshot {
        peer_device_id: String,
        policy_generation: i64,
    },
    RecordOutboundScan {
        peer_device_id: String,
        policy_generation: i64,
        scanned_through: i64,
    },
    AcknowledgeOutbound {
        peer_device_id: String,
        policy_generation: i64,
        position: i64,
        ack: OutboundAck,
    },
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CommittedFeedProgressCommand {
    operation: CommittedFeedProgressOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommittedFeedProgress {
    Outbound(OutboundProgress),
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReceiveInboxReceiptCommand {
    peer_device_id: String,
    message_id: String,
    payload_json: String,
}

impl ReceiveInboxReceiptCommand {
    pub(crate) fn new(peer_device_id: String, message_id: String, payload_json: String) -> Self {
        Self {
            peer_device_id,
            message_id,
            payload_json,
        }
    }
}

impl sealed::Sealed for ReceiveInboxReceiptCommand {}

impl DbCommand for ReceiveInboxReceiptCommand {
    type Output = crate::db::InboxState;
    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_sync_peer_matches(&self.peer_device_id)?;
        super::device_registry_sync::require_registry_peer_admitted(
            transaction,
            &self.peer_device_id,
        )?;
        let removed: Vec<String> = crate::db::get_sync_kv(transaction, "sync.removed_peers")
            .map_err(DbError::from_application_error)?
            .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
            .transpose()
            .map_err(DbError::from_application_error)?
            .unwrap_or_default();
        if removed
            .iter()
            .any(|removed_id| removed_id == &self.peer_device_id)
        {
            return Err(DbError::Permission("sync_peer_removed".into()));
        }
        crate::db::receive_inbox_payload(
            transaction,
            &self.peer_device_id,
            &self.message_id,
            &self.payload_json,
        )
        .map_err(DbError::from_application_error)
    }
}

impl CommittedFeedProgressCommand {
    pub(crate) fn new(operation: CommittedFeedProgressOperation) -> Self {
        Self { operation }
    }
}

impl sealed::Sealed for CommittedFeedProgressCommand {}

impl DbCommand for CommittedFeedProgressCommand {
    type Output = CommittedFeedProgress;
    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        let peer_device_id = match &self.operation {
            CommittedFeedProgressOperation::BeginOutboundPolicy { peer_device_id, .. }
            | CommittedFeedProgressOperation::CompleteOutboundSnapshot { peer_device_id, .. }
            | CommittedFeedProgressOperation::RecordOutboundScan { peer_device_id, .. }
            | CommittedFeedProgressOperation::AcknowledgeOutbound { peer_device_id, .. } => {
                peer_device_id
            }
        };
        context.require_sync_peer_matches(peer_device_id)?;
        use CommittedFeedProgressOperation as Op;
        match self.operation {
            Op::BeginOutboundPolicy {
                peer_device_id,
                policy_generation,
            } => crate::db::begin_outbound_policy(transaction, &peer_device_id, policy_generation)
                .map(CommittedFeedProgress::Outbound),
            Op::CompleteOutboundSnapshot {
                peer_device_id,
                policy_generation,
            } => crate::db::complete_outbound_snapshot(
                transaction,
                &peer_device_id,
                policy_generation,
            )
            .map(CommittedFeedProgress::Outbound),
            Op::RecordOutboundScan {
                peer_device_id,
                policy_generation,
                scanned_through,
            } => crate::db::record_outbound_scan(
                transaction,
                &peer_device_id,
                policy_generation,
                scanned_through,
            )
            .map(CommittedFeedProgress::Outbound),
            Op::AcknowledgeOutbound {
                peer_device_id,
                policy_generation,
                position,
                ack,
            } => crate::db::acknowledge_outbound(
                transaction,
                &peer_device_id,
                policy_generation,
                position,
                ack,
            )
            .map(CommittedFeedProgress::Outbound),
        }
        .map_err(DbError::Storage)
    }
}

struct SqliteInboundSyncEntitySession<'transaction, 'connection>(
    &'transaction Transaction<'connection>,
);

impl crate::application::sync_storage::SyncEntityMutationSession
    for SqliteInboundSyncEntitySession<'_, '_>
{
    fn apply_entity(
        &mut self,
        entity: &SyncEntity,
        authenticated_peer_id: Option<&str>,
    ) -> Result<
        (
            crate::application::sync_storage::InboundApplyOutcome,
            Vec<ReplayNotice>,
        ),
        String,
    > {
        match authenticated_peer_id {
            Some(peer_id) => SqliteStorageBackend::apply_authenticated_entity_in_transaction(
                self.0, entity, peer_id,
            ),
            None => SqliteStorageBackend::apply_entity_in_transaction(self.0, entity),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EnsureUsageSequenceMigratedCommand {
    device_id: String,
}

impl EnsureUsageSequenceMigratedCommand {
    pub(crate) fn new(device_id: String) -> Self {
        Self { device_id }
    }
}

impl sealed::Sealed for EnsureUsageSequenceMigratedCommand {}

impl DbCommand for EnsureUsageSequenceMigratedCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        ensure_usage_sequence_migrated(transaction, &self.device_id).map_err(DbError::Storage)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApplySyncEntityCommand {
    entity: SyncEntity,
}

impl ApplySyncEntityCommand {
    pub(crate) fn new(entity: SyncEntity) -> Self {
        Self { entity }
    }
}

impl sealed::Sealed for ApplySyncEntityCommand {}

impl DbCommand for ApplySyncEntityCommand {
    type Output = Vec<ReplayNotice>;

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        let mut session = SqliteInboundSyncEntitySession(transaction);
        let (_, notices) = crate::application::sync_storage::apply_inbound_sync_entity(
            &mut session,
            &self.entity,
            None,
        )
        .map_err(DbError::from_application_error)?;
        Ok(notices)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApplyAuthenticatedSyncEntityCommand {
    entity: SyncEntity,
    message_id: String,
}

impl ApplyAuthenticatedSyncEntityCommand {
    pub(crate) fn with_message_id(entity: SyncEntity, message_id: String) -> Self {
        Self { entity, message_id }
    }
}

impl sealed::Sealed for ApplyAuthenticatedSyncEntityCommand {}

impl DbCommand for ApplyAuthenticatedSyncEntityCommand {
    type Output = (
        crate::application::sync_storage::InboundApplyOutcome,
        Vec<ReplayNotice>,
    );

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        let peer_id = context.authenticated_peer_id()?;
        super::device_registry_sync::require_registry_peer_admitted(transaction, peer_id)?;
        let message_id = self.message_id;
        let payload = serde_json::to_string(&self.entity)
            .map_err(|error| DbError::from_application_error(error.to_string()))?;
        let inbox_state =
            crate::db::receive_inbox_payload(transaction, peer_id, &message_id, &payload)
                .map_err(DbError::from_application_error)?;
        if inbox_state == crate::db::InboxState::Applied {
            return Ok((
                crate::application::sync_storage::InboundApplyOutcome::Applied,
                Vec::new(),
            ));
        }
        if inbox_state == crate::db::InboxState::Pending {
            return Ok((
                crate::application::sync_storage::InboundApplyOutcome::Pending,
                Vec::new(),
            ));
        }
        if inbox_state != crate::db::InboxState::Received {
            return Err(DbError::from_application_error(
                "sync_inbox_payload_not_ready".into(),
            ));
        }
        let mut session = SqliteInboundSyncEntitySession(transaction);
        let (outcome, notices) = crate::application::sync_storage::apply_inbound_sync_entity(
            &mut session,
            &self.entity,
            Some(peer_id),
        )
        .map_err(DbError::from_application_error)?;
        let inbox_state = match outcome {
            crate::application::sync_storage::InboundApplyOutcome::Applied
            | crate::application::sync_storage::InboundApplyOutcome::Stale => {
                crate::db::InboxState::Applied
            }
            crate::application::sync_storage::InboundApplyOutcome::Pending => {
                crate::db::InboxState::Pending
            }
        };
        crate::db::set_inbox_state(transaction, peer_id, &message_id, inbox_state)
            .map_err(DbError::from_application_error)?;
        Ok((outcome, notices))
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetSyncKeyValueCommand {
    key: String,
    value: String,
}

impl SetSyncKeyValueCommand {
    pub(crate) fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl sealed::Sealed for SetSyncKeyValueCommand {}

impl DbCommand for SetSyncKeyValueCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::SyncLive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_system_sync()?;
        set_sync_kv(transaction, &self.key, &self.value).map_err(DbError::Storage)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SetLocalSyncKeyValueCommand {
    key: String,
    value: String,
}

impl SetLocalSyncKeyValueCommand {
    pub(crate) fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl sealed::Sealed for SetLocalSyncKeyValueCommand {}

impl DbCommand for SetLocalSyncKeyValueCommand {
    type Output = ();

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        set_sync_kv(transaction, &self.key, &self.value).map_err(DbError::Storage)
    }
}
