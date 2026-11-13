use super::Transaction;

pub fn apply_in(
    transaction: &Transaction<'_>,
    peer_id: &str,
    write: crate::task_object_write::AuthenticatedCoreTaskWrite,
) -> Result<
    crate::task_object_write::RemoteCoreTaskOutcome,
    crate::task_object_write::AuthenticatedCoreTaskWriteError,
> {
    let object_session = crate::object_write::SqliteObjectWriteSession::new(transaction);
    let task_session = crate::task_merge::SqliteTaskMutationSession::new(transaction);
    let auth_session = SqliteAuthenticatedCoreTaskWriteSession {
        connection: transaction,
    };
    crate::task_object_write::apply_authenticated_core_task_write(
        &object_session,
        &task_session,
        &auth_session,
        peer_id,
        write,
    )
}

pub struct SqliteAuthenticatedCoreTaskWriteSession<'connection> {
    connection: &'connection rusqlite::Connection,
}

impl SqliteAuthenticatedCoreTaskWriteSession<'_> {
    pub(crate) fn load_sync_profile_for_sync(
        &self,
        local_device_id: &str,
    ) -> Result<Option<domain::sync_policy::SyncProfile>, String> {
        crate::data_platform::load_sync_profile_for_sync(self.connection, local_device_id)
    }

    pub(crate) fn is_peer_admitted(&self, peer_id: &str) -> Result<bool, String> {
        Ok(crate::device_registry::is_device_admitted(self.connection, peer_id)?.unwrap_or(false))
    }

    pub(crate) fn receive_inbox_payload(
        &self,
        peer_id: &str,
        message_id: &str,
        payload_json: &str,
    ) -> Result<crate::task_object_write::InboxReceiptState, String> {
        crate::committed_feed::receive_inbox_payload(
            self.connection,
            peer_id,
            message_id,
            payload_json,
        )
        .map(|state| match state {
            crate::committed_feed::InboxState::Received => {
                crate::task_object_write::InboxReceiptState::Received
            }
            crate::committed_feed::InboxState::Pending => {
                crate::task_object_write::InboxReceiptState::Pending
            }
            crate::committed_feed::InboxState::Applied => {
                crate::task_object_write::InboxReceiptState::Applied
            }
            crate::committed_feed::InboxState::Rejected => {
                crate::task_object_write::InboxReceiptState::Rejected
            }
        })
    }

    pub(crate) fn mark_inbox_applied(&self, peer_id: &str, message_id: &str) -> Result<(), String> {
        crate::committed_feed::set_inbox_state(
            self.connection,
            peer_id,
            message_id,
            crate::committed_feed::InboxState::Applied,
        )
    }
}
