use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use ark_application::sync_storage::PolicyIdentity;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReceiveInboxReceiptCommand {
    peer_device_id: String,
    message_id: String,
    payload_json: String,
    local_device_id: Option<String>,
    policy_identity: Option<PolicyIdentity>,
}

impl ReceiveInboxReceiptCommand {
    pub(crate) fn new(peer_device_id: String, message_id: String, payload_json: String) -> Self {
        Self {
            peer_device_id,
            message_id,
            payload_json,
            local_device_id: None,
            policy_identity: None,
        }
    }

    pub(crate) fn with_policy_identity(
        peer_device_id: String,
        message_id: String,
        payload_json: String,
        local_device_id: String,
        policy_identity: PolicyIdentity,
    ) -> Self {
        Self {
            peer_device_id,
            message_id,
            payload_json,
            local_device_id: Some(local_device_id),
            policy_identity: Some(policy_identity),
        }
    }
}

pub(crate) fn require_sync_policy_identity(
    transaction: &Transaction<'_>,
    local_device_id: Option<&str>,
    expected: Option<&PolicyIdentity>,
) -> Result<(), DbError> {
    if let Some(expected) = expected {
        let local_device_id = local_device_id.ok_or_else(|| {
            DbError::Permission("sync_policy_identity_missing_local_device".into())
        })?;
        let profile =
            ark_store::data_platform::load_sync_profile_for_sync(transaction, local_device_id)
                .map_err(DbError::from_application_error)?;
        let actual = crate::protocol::sync_policy_identity(profile.as_ref());
        if &actual != expected {
            return Err(DbError::Permission(
                "stale_or_unnegotiated_sync_policy".into(),
            ));
        }
    }
    Ok(())
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
        require_sync_policy_identity(
            transaction,
            self.local_device_id.as_deref(),
            self.policy_identity.as_ref(),
        )?;
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
