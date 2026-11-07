use super::{SqliteWriter, StoreTransactionFailure, WriteCapabilities};

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
        mutation: crate::tracked_app_mutation::TrackedAppMutation,
    ) -> Result<domain::types::SyncEntity, String> {
        let session =
            crate::tracked_app_mutation::SqliteTrackedAppMutationSession::new(self.transaction);
        crate::tracked_app_mutation::apply_tracked_app_mutation(&session, mutation)
    }
}

impl SqliteWriter {
    pub fn apply_tracked_app_mutation(
        &mut self,
        mutation: crate::tracked_app_mutation::TrackedAppMutation,
    ) -> Result<domain::types::SyncEntity, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            capabilities.tracked_app_mutation().apply(mutation)
        })
    }
}
