use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use super::inbound_policy::require_sync_policy_identity;
use crate::canonical_types::pending::ReplayNotice;
use crate::db::{ensure_usage_sequence_migrated, SqliteStorageBackend};
use crate::types::SyncEntity;
use ark_application::sync_storage::{OutboundAck, OutboundProgress, PolicyIdentity};
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
    BeginOutboundPolicyIdentity {
        peer_device_id: String,
        identity: PolicyIdentity,
    },
    CompleteOutboundSnapshotIdentity {
        peer_device_id: String,
        identity: PolicyIdentity,
    },
    RecordOutboundScanIdentity {
        peer_device_id: String,
        identity: PolicyIdentity,
        scanned_through: i64,
    },
    AcknowledgeOutboundIdentity {
        peer_device_id: String,
        identity: PolicyIdentity,
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
            | CommittedFeedProgressOperation::AcknowledgeOutbound { peer_device_id, .. }
            | CommittedFeedProgressOperation::BeginOutboundPolicyIdentity {
                peer_device_id, ..
            }
            | CommittedFeedProgressOperation::CompleteOutboundSnapshotIdentity {
                peer_device_id,
                ..
            }
            | CommittedFeedProgressOperation::RecordOutboundScanIdentity {
                peer_device_id, ..
            }
            | CommittedFeedProgressOperation::AcknowledgeOutboundIdentity {
                peer_device_id, ..
            } => peer_device_id,
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
            Op::BeginOutboundPolicyIdentity {
                peer_device_id,
                identity,
            } => crate::db::begin_outbound_policy_identity(transaction, &peer_device_id, &identity)
                .map(CommittedFeedProgress::Outbound),
            Op::CompleteOutboundSnapshotIdentity {
                peer_device_id,
                identity,
            } => crate::db::complete_outbound_snapshot_identity(
                transaction,
                &peer_device_id,
                &identity,
            )
            .map(CommittedFeedProgress::Outbound),
            Op::RecordOutboundScanIdentity {
                peer_device_id,
                identity,
                scanned_through,
            } => crate::db::record_outbound_scan_identity(
                transaction,
                &peer_device_id,
                &identity,
                scanned_through,
            )
            .map(CommittedFeedProgress::Outbound),
            Op::AcknowledgeOutboundIdentity {
                peer_device_id,
                identity,
                position,
                ack,
            } => crate::db::acknowledge_outbound_identity(
                transaction,
                &peer_device_id,
                &identity,
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

impl ark_application::sync_storage::SyncEntityMutationSession
    for SqliteInboundSyncEntitySession<'_, '_>
{
    fn apply_entity(
        &mut self,
        entity: &SyncEntity,
        authenticated_peer_id: Option<&str>,
    ) -> Result<
        (
            ark_application::sync_storage::InboundApplyOutcome,
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
        let (_, notices) = ark_application::sync_storage::apply_inbound_sync_entity(
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
    local_device_id: Option<String>,
    policy_identity: Option<PolicyIdentity>,
}

impl ApplyAuthenticatedSyncEntityCommand {
    pub(crate) fn with_message_id(entity: SyncEntity, message_id: String) -> Self {
        Self {
            entity,
            message_id,
            local_device_id: None,
            policy_identity: None,
        }
    }

    pub(crate) fn with_policy_identity(
        entity: SyncEntity,
        message_id: String,
        local_device_id: String,
        policy_identity: PolicyIdentity,
    ) -> Self {
        Self {
            entity,
            message_id,
            local_device_id: Some(local_device_id),
            policy_identity: Some(policy_identity),
        }
    }
}

impl sealed::Sealed for ApplyAuthenticatedSyncEntityCommand {}

impl DbCommand for ApplyAuthenticatedSyncEntityCommand {
    type Output = (
        ark_application::sync_storage::InboundApplyOutcome,
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
        require_sync_policy_identity(
            transaction,
            self.local_device_id.as_deref(),
            self.policy_identity.as_ref(),
        )?;
        super::device_registry_sync::require_registry_peer_admitted(transaction, peer_id)?;
        let message_id = self.message_id;
        let payload = serde_json::to_string(&self.entity)
            .map_err(|error| DbError::from_application_error(error.to_string()))?;
        let inbox_state =
            crate::db::receive_inbox_payload(transaction, peer_id, &message_id, &payload)
                .map_err(DbError::from_application_error)?;
        if inbox_state == crate::db::InboxState::Applied {
            return Ok((
                ark_application::sync_storage::InboundApplyOutcome::Applied,
                Vec::new(),
            ));
        }
        if inbox_state == crate::db::InboxState::Pending {
            return Ok((
                ark_application::sync_storage::InboundApplyOutcome::Pending,
                Vec::new(),
            ));
        }
        if inbox_state != crate::db::InboxState::Received {
            return Err(DbError::from_application_error(
                "sync_inbox_payload_not_ready".into(),
            ));
        }
        let mut session = SqliteInboundSyncEntitySession(transaction);
        let (outcome, notices) = ark_application::sync_storage::apply_inbound_sync_entity(
            &mut session,
            &self.entity,
            Some(peer_id),
        )
        .map_err(DbError::from_application_error)?;
        let inbox_state = match outcome {
            ark_application::sync_storage::InboundApplyOutcome::Applied
            | ark_application::sync_storage::InboundApplyOutcome::Stale => {
                crate::db::InboxState::Applied
            }
            ark_application::sync_storage::InboundApplyOutcome::Pending => {
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
    local_device_id: Option<String>,
    policy_identity: Option<PolicyIdentity>,
}

impl SetSyncKeyValueCommand {
    pub(crate) fn new(key: String, value: String) -> Self {
        Self {
            key,
            value,
            local_device_id: None,
            policy_identity: None,
        }
    }

    pub(crate) fn with_policy_identity(
        key: String,
        value: String,
        local_device_id: String,
        policy_identity: PolicyIdentity,
    ) -> Self {
        Self {
            key,
            value,
            local_device_id: Some(local_device_id),
            policy_identity: Some(policy_identity),
        }
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
        require_sync_policy_identity(
            transaction,
            self.local_device_id.as_deref(),
            self.policy_identity.as_ref(),
        )?;
        ark_store::sync_kv::set_sync_kv(transaction, &self.key, &self.value)
            .map_err(DbError::Storage)
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
        ark_store::sync_kv::set_sync_kv(transaction, &self.key, &self.value)
            .map_err(DbError::Storage)
    }
}
