use ark_application::integration_admin::{IntegrationAdminOutput, IntegrationAdminWrite};
#[cfg(debug_assertions)]
use rusqlite::Connection;
use rusqlite::Transaction;

#[cfg(debug_assertions)]
use super::TransactionFailure;
use super::{SqliteWriter, StoreTransactionFailure, WriteCapabilities};

pub struct IntegrationAdminWrites<'scope, 'connection> {
    transaction: &'scope Transaction<'connection>,
}

impl WriteCapabilities<'_, '_> {
    pub fn integration_admin(&mut self) -> IntegrationAdminWrites<'_, '_> {
        IntegrationAdminWrites {
            transaction: self.transaction,
        }
    }
}

impl IntegrationAdminWrites<'_, '_> {
    pub fn apply(
        &mut self,
        write: IntegrationAdminWrite,
    ) -> Result<IntegrationAdminOutput, String> {
        let store =
            crate::integration_replication::SqliteIntegrationAdminStore::new(self.transaction);
        ark_application::integration_admin::apply(&store, write)
    }
}

impl SqliteWriter {
    pub fn apply_integration_admin(
        &mut self,
        write: IntegrationAdminWrite,
    ) -> Result<IntegrationAdminOutput, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|caps| caps.integration_admin().apply(write))
    }
}

#[cfg(debug_assertions)]
pub fn apply_integration_admin_on_connection(
    connection: &mut Connection,
    write: IntegrationAdminWrite,
) -> Result<IntegrationAdminOutput, TransactionFailure<String>> {
    SqliteWriter::immediate_on_connection(connection, |tx| {
        let store = crate::integration_replication::SqliteIntegrationAdminStore::new(tx);
        ark_application::integration_admin::apply(&store, write)
    })
}
