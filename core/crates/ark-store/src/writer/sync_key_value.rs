use super::{Connection, SqliteWriter, TransactionFailure};

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
}
