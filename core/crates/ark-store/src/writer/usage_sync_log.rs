use super::{Connection, SqliteWriter, StoreTransactionFailure, TransactionFailure};

impl SqliteWriter {
    pub fn compact_usage_sync_log(
        &mut self,
        own_device_id: Option<&str>,
        cutoff_wall_time: &str,
        batch_limit: i64,
    ) -> Result<usize, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            crate::usage_sync_log::compact_usage_sync_log(
                capabilities.transaction,
                own_device_id,
                cutoff_wall_time,
                batch_limit,
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn compact_usage_sync_log_on_connection(
        connection: &mut Connection,
        own_device_id: Option<&str>,
        cutoff_wall_time: &str,
        batch_limit: i64,
    ) -> Result<usize, TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::usage_sync_log::compact_usage_sync_log(
                transaction,
                own_device_id,
                cutoff_wall_time,
                batch_limit,
            )
        })
    }
}
