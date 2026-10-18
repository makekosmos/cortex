use super::super::{sealed, CommandClass, CommandContext, DbError, DbQuery};

#[derive(serde::Serialize)]
pub(crate) enum SystemReadQuery {
    SyncKeyValue(String),
    ValidateSnapshot { db_path: String, backup_id: String },
}

impl sealed::Sealed for SystemReadQuery {}

impl DbQuery for SystemReadQuery {
    type Output = serde_json::Value;

    fn class(&self) -> CommandClass {
        CommandClass::Interactive
    }

    fn weight(&self) -> usize {
        super::serialized_weight(self)
    }

    fn read(
        self,
        context: &CommandContext,
        connection: &rusqlite::Connection,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        match self {
            Self::SyncKeyValue(key) => ark_store::sync_kv::get_sync_kv(connection, &key)
                .map(|value| serde_json::json!(value))
                .map_err(DbError::Storage),
            Self::ValidateSnapshot { db_path, backup_id } => {
                let validation = crate::db::validate_snapshot(connection, &db_path, &backup_id)
                    .map_err(DbError::Storage)?;
                serde_json::to_value(validation)
                    .map_err(|error| DbError::Storage(error.to_string()))
            }
        }
    }
}
