use super::{SqliteWriter, StoreTransactionFailure, WriteCapabilities};
use rusqlite::Connection;

pub struct TrackedAppMutationWrites<'scope, 'connection> {
    transaction: &'scope rusqlite::Transaction<'connection>,
}

impl WriteCapabilities<'_, '_> {
    pub fn tracked_app_mutation(&mut self) -> TrackedAppMutationWrites<'_, '_> {
        TrackedAppMutationWrites {
            transaction: self.transaction,
        }
    }
}

impl TrackedAppMutationWrites<'_, '_> {
    pub fn apply(
        &mut self,
        mutation: ark_application::tracked_app_mutation::TrackedAppMutation,
    ) -> Result<ark_domain::types::SyncEntity, String> {
        let session =
            crate::tracked_app_mutation::SqliteTrackedAppMutationSession::new(self.transaction);
        ark_application::tracked_app_mutation::apply_tracked_app_mutation(&session, mutation)
    }
}

impl SqliteWriter {
    pub fn apply_tracked_app_mutation(
        &mut self,
        mutation: ark_application::tracked_app_mutation::TrackedAppMutation,
    ) -> Result<ark_domain::types::SyncEntity, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            capabilities.tracked_app_mutation().apply(mutation)
        })
    }

    pub fn apply_tracked_app_mutation_on_connection(
        connection: &mut Connection,
        mutation: ark_application::tracked_app_mutation::TrackedAppMutation,
    ) -> Result<ark_domain::types::SyncEntity, StoreTransactionFailure<String>> {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            capabilities.tracked_app_mutation().apply(mutation)
        })
    }
}
