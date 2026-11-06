use domain::canonical_type::TypeRegistration;
use domain::sync_storage::ReplayNotice;
use rusqlite::Transaction;

pub struct SqliteTypeMutationSession<'tx, 'connection> {
    transaction: &'tx Transaction<'connection>,
}

impl<'tx, 'connection> SqliteTypeMutationSession<'tx, 'connection> {
    pub(crate) fn new(transaction: &'tx Transaction<'connection>) -> Self {
        Self { transaction }
    }
}

impl SqliteTypeMutationSession<'_, '_> {
    pub(crate) fn register_application_type(
        &self,
        registration: &TypeRegistration,
    ) -> Result<Vec<ReplayNotice>, String> {
        crate::type_registry::register_application_type(self.transaction, registration)
    }
}
