use super::{Connection, SqliteWriter, StoreTransactionFailure, TransactionFailure};
use crate::data_platform::SyncProfileWriteError;
use ark_application::sync_policy::{AcceptedPeerSyncPolicy, SyncProfile, SyncProfileRule};

impl SqliteWriter {
    pub fn save_local_sync_profile(
        &mut self,
        device_id: &str,
        profile_id: &str,
        name: &str,
        active: bool,
        expected_revision: Option<i64>,
        rules: &[SyncProfileRule],
    ) -> Result<SyncProfile, StoreTransactionFailure<SyncProfileWriteError>> {
        self.write_with_capabilities(|capabilities| {
            crate::data_platform::upsert_sync_profile(
                capabilities.transaction,
                device_id,
                profile_id,
                name,
                active,
                expected_revision,
                rules,
            )
        })
    }

    pub fn accept_peer_sync_policy(
        &mut self,
        peer_device_id: &str,
        revision: i64,
        digest: &str,
        active: bool,
        rules: &[SyncProfileRule],
    ) -> Result<AcceptedPeerSyncPolicy, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            crate::data_platform::accept_peer_sync_policy(
                capabilities.transaction,
                peer_device_id,
                revision,
                digest,
                active,
                rules,
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn save_local_sync_profile_on_connection(
        connection: &mut Connection,
        device_id: &str,
        profile_id: &str,
        name: &str,
        active: bool,
        expected_revision: Option<i64>,
        rules: &[SyncProfileRule],
    ) -> Result<SyncProfile, TransactionFailure<SyncProfileWriteError>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::data_platform::upsert_sync_profile(
                transaction,
                device_id,
                profile_id,
                name,
                active,
                expected_revision,
                rules,
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn accept_peer_sync_policy_on_connection(
        connection: &mut Connection,
        peer_device_id: &str,
        revision: i64,
        digest: &str,
        active: bool,
        rules: &[SyncProfileRule],
    ) -> Result<AcceptedPeerSyncPolicy, TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::data_platform::accept_peer_sync_policy(
                transaction,
                peer_device_id,
                revision,
                digest,
                active,
                rules,
            )
        })
    }
}
