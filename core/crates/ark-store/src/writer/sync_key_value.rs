use super::{Connection, SqliteWriter, Transaction, TransactionFailure};
use crate::writer::StoreTransactionFailure;
use ark_application::sync_storage::{SyncProgressWrite, SyncProgressWriteError};

impl SqliteWriter {
    pub fn set_local_sync_key_value(
        &mut self,
        key: &str,
        value: &str,
    ) -> Result<(), TransactionFailure<String>> {
        Self::immediate_on_connection(&mut self.connection, |transaction| {
            crate::sync_kv::set_sync_kv(transaction, key, value)
        })
    }

    pub fn set_local_sync_key_value_on_connection(
        connection: &mut Connection,
        key: &str,
        value: &str,
    ) -> Result<(), TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::sync_kv::set_sync_kv(transaction, key, value)
        })
    }

    pub fn set_sync_progress(
        &mut self,
        write: SyncProgressWrite,
    ) -> Result<(), StoreTransactionFailure<SyncProgressWriteError>> {
        self.write_with_capabilities(|capabilities| {
            apply_sync_progress_write(capabilities.transaction, write)
        })
    }

    pub fn set_sync_progress_on_connection(
        connection: &mut Connection,
        write: SyncProgressWrite,
    ) -> Result<(), TransactionFailure<SyncProgressWriteError>> {
        Self::immediate_on_connection(connection, |transaction| {
            apply_sync_progress_write(transaction, write)
        })
    }
}

fn apply_sync_progress_write(
    transaction: &Transaction<'_>,
    write: SyncProgressWrite,
) -> Result<(), SyncProgressWriteError> {
    struct Session<'scope, 'connection>(&'scope Transaction<'connection>);

    impl ark_application::sync_storage::SyncProgressWriteSession for Session<'_, '_> {
        fn load_sync_profile_for_sync(
            &self,
            local_device_id: &str,
        ) -> Result<Option<ark_application::sync_policy::SyncProfile>, String> {
            crate::data_platform::load_sync_profile_for_sync(self.0, local_device_id)
        }

        fn set_sync_value(&self, key: &str, value: &str) -> Result<(), String> {
            crate::sync_kv::set_sync_kv(self.0, key, value)
        }
    }

    let session = Session(transaction);
    ark_application::sync_storage::apply_sync_progress_write(&session, write)
}
