use super::{SqliteWriter, StoreTransactionFailure, WriteCapabilities};

impl SqliteWriter {
    pub fn register_application_type(
        &mut self,
        registration: ark_domain::canonical_type::TypeRegistration,
    ) -> Result<Vec<ark_domain::sync_storage::ReplayNotice>, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            let session = type_mutation_session(capabilities);
            crate::type_mutation::register_application_type(&session, &registration)
        })
    }
}

fn type_mutation_session<'scope, 'connection>(
    capabilities: &WriteCapabilities<'scope, 'connection>,
) -> crate::type_registry::mutation_session::SqliteTypeMutationSession<'scope, 'connection> {
    crate::type_registry::mutation_session::SqliteTypeMutationSession::new(capabilities.transaction)
}
