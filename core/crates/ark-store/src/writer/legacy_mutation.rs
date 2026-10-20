#[cfg(debug_assertions)]
use super::Connection;
use super::{SqliteWriter, TransactionFailure};

impl SqliteWriter {
    pub fn apply_local_legacy_graph_write(
        &mut self,
        records: &[ark_application::canonical_compatibility::LegacyRecord],
        source_kind: &str,
        device_id: Option<String>,
    ) -> Result<Vec<ark_domain::types::SyncEntity>, TransactionFailure<String>> {
        self.immediate(|transaction| {
            crate::legacy_mutation::apply_local_legacy_graph_write_in(
                transaction,
                records,
                source_kind,
                device_id.as_deref(),
            )
        })
    }

    pub fn apply_local_legacy_delete(
        &mut self,
        id: &str,
        expected_type_id: &str,
        device_id: Option<String>,
    ) -> Result<ark_domain::types::SyncEntity, TransactionFailure<String>> {
        self.immediate(|transaction| {
            crate::legacy_mutation::apply_local_legacy_delete_in(
                transaction,
                id,
                expected_type_id,
                device_id.as_deref(),
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn apply_local_legacy_graph_write_on_connection(
        connection: &mut Connection,
        records: &[ark_application::canonical_compatibility::LegacyRecord],
        source_kind: &str,
        device_id: Option<String>,
    ) -> Result<Vec<ark_domain::types::SyncEntity>, TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::legacy_mutation::apply_local_legacy_graph_write_in(
                transaction,
                records,
                source_kind,
                device_id.as_deref(),
            )
        })
    }

    #[cfg(debug_assertions)]
    pub fn apply_local_legacy_delete_on_connection(
        connection: &mut Connection,
        id: &str,
        expected_type_id: &str,
        device_id: Option<String>,
    ) -> Result<ark_domain::types::SyncEntity, TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::legacy_mutation::apply_local_legacy_delete_in(
                transaction,
                id,
                expected_type_id,
                device_id.as_deref(),
            )
        })
    }
}
