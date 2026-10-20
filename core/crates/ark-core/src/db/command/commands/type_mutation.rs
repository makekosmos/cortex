use super::super::{sealed, CommandClass, CommandContext, DbCommand, DbError};
use crate::application::type_mutation::{
    delete_object_type, register_package_types, upsert_object_type, TypeMutationCommit,
};
use crate::db::SqliteTypeMutationSession;
use crate::type_registry::TypeRegistration;
use crate::types::ObjectType;
use ark_application::sync_storage::ReplayNotice;
use rusqlite::Transaction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RegisterPackageTypesCommand {
    registrations: Vec<TypeRegistration>,
}

impl RegisterPackageTypesCommand {
    pub(crate) fn new(registrations: Vec<TypeRegistration>) -> Self {
        Self { registrations }
    }
}

impl sealed::Sealed for RegisterPackageTypesCommand {}

impl DbCommand for RegisterPackageTypesCommand {
    type Output = Vec<ReplayNotice>;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let session = SqliteTypeMutationSession::new(transaction);
        register_package_types(&session, &self.registrations)
            .map_err(DbError::from_application_error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpsertObjectTypeCommand {
    object_type: ObjectType,
    device_id: Option<String>,
}

impl UpsertObjectTypeCommand {
    pub(crate) fn new(object_type: ObjectType, device_id: Option<String>) -> Self {
        Self {
            object_type,
            device_id,
        }
    }
}

impl sealed::Sealed for UpsertObjectTypeCommand {}

impl DbCommand for UpsertObjectTypeCommand {
    type Output = TypeMutationCommit;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let session = SqliteTypeMutationSession::new(transaction);
        upsert_object_type(&session, self.object_type, self.device_id.as_deref())
            .map_err(DbError::from_application_error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DeleteObjectTypeCommand {
    id: String,
    device_id: Option<String>,
}

impl DeleteObjectTypeCommand {
    pub(crate) fn new(id: String, device_id: Option<String>) -> Self {
        Self { id, device_id }
    }
}

impl sealed::Sealed for DeleteObjectTypeCommand {}

impl DbCommand for DeleteObjectTypeCommand {
    type Output = TypeMutationCommit;

    const CLASS: CommandClass = CommandClass::Interactive;

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn execute(
        self,
        context: &CommandContext,
        transaction: &Transaction<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        let session = SqliteTypeMutationSession::new(transaction);
        delete_object_type(&session, self.id, self.device_id.as_deref())
            .map_err(DbError::from_application_error)
    }
}
