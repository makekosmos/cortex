use super::{Connection, SqliteWriter, TransactionFailure};

impl SqliteWriter {
    pub fn ensure_usage_sequence_migrated(
        &mut self,
        device_id: &str,
    ) -> Result<(), TransactionFailure<String>> {
        Self::immediate_on_connection(&mut self.connection, |transaction| {
            crate::usage_sync_log::ensure_usage_sequence_migrated(transaction, device_id)
        })
    }

    pub fn ensure_usage_sequence_migrated_on_connection(
        connection: &mut Connection,
        device_id: &str,
    ) -> Result<(), TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::usage_sync_log::ensure_usage_sequence_migrated(transaction, device_id)
        })
    }
}
