use super::{Connection, SqliteWriter, StoreTransactionFailure, WriteCapabilities};

impl SqliteWriter {
    pub fn register_package_types(
        &mut self,
        registrations: Vec<ark_domain::canonical_type::TypeRegistration>,
    ) -> Result<Vec<ark_domain::sync_storage::ReplayNotice>, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            let session = type_mutation_session(capabilities);
            crate::type_mutation::register_package_types(&session, &registrations)
        })
    }

    pub fn register_application_type(
        &mut self,
        registration: ark_domain::canonical_type::TypeRegistration,
    ) -> Result<Vec<ark_domain::sync_storage::ReplayNotice>, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            let session = type_mutation_session(capabilities);
            crate::type_mutation::register_application_type(&session, &registration)
        })
    }

    pub fn upsert_object_type(
        &mut self,
        object_type: ark_domain::types::ObjectType,
        device_id: Option<String>,
    ) -> Result<crate::type_mutation::TypeMutationCommit, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            let session = type_mutation_session(capabilities);
            crate::type_mutation::upsert_object_type(&session, object_type, device_id.as_deref())
        })
    }

    pub fn delete_object_type(
        &mut self,
        id: String,
        device_id: Option<String>,
    ) -> Result<crate::type_mutation::TypeMutationCommit, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            let session = type_mutation_session(capabilities);
            crate::type_mutation::delete_object_type(&session, id, device_id.as_deref())
        })
    }

    pub fn register_package_types_on_connection(
        connection: &mut Connection,
        registrations: Vec<ark_domain::canonical_type::TypeRegistration>,
    ) -> Result<Vec<ark_domain::sync_storage::ReplayNotice>, StoreTransactionFailure<String>> {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            let session = type_mutation_session(capabilities);
            crate::type_mutation::register_package_types(&session, &registrations)
        })
    }

    pub fn upsert_object_type_on_connection(
        connection: &mut Connection,
        object_type: ark_domain::types::ObjectType,
        device_id: Option<String>,
    ) -> Result<crate::type_mutation::TypeMutationCommit, StoreTransactionFailure<String>> {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            let session = type_mutation_session(capabilities);
            crate::type_mutation::upsert_object_type(&session, object_type, device_id.as_deref())
        })
    }

    pub fn delete_object_type_on_connection(
        connection: &mut Connection,
        id: String,
        device_id: Option<String>,
    ) -> Result<crate::type_mutation::TypeMutationCommit, StoreTransactionFailure<String>> {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            let session = type_mutation_session(capabilities);
            crate::type_mutation::delete_object_type(&session, id, device_id.as_deref())
        })
    }
}

fn type_mutation_session<'scope, 'connection>(
    capabilities: &WriteCapabilities<'scope, 'connection>,
) -> crate::type_registry::mutation_session::SqliteTypeMutationSession<'scope, 'connection> {
    crate::type_registry::mutation_session::SqliteTypeMutationSession::new(capabilities.transaction)
}
