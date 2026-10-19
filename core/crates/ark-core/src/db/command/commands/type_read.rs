use super::super::{sealed, CommandClass, CommandContext, DbError, DbQuery};

fn serialize<T: serde::Serialize>(value: Result<T, String>) -> Result<serde_json::Value, DbError> {
    let value = value.map_err(DbError::Storage)?;
    serde_json::to_value(value).map_err(|error| DbError::Storage(error.to_string()))
}

#[derive(serde::Serialize)]
pub(crate) enum TypeReadQuery {
    ListSummaries,
    Get {
        type_id: String,
        version: Option<String>,
    },
    ListVersions(String),
    ResolveAlias(String),
    ListObjectTypes,
    GetObjectType(String),
    ListObjectLinks,
}

impl sealed::Sealed for TypeReadQuery {}

impl DbQuery for TypeReadQuery {
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
            Self::ListSummaries => serialize(crate::type_registry::list_type_summaries(connection)),
            Self::Get { type_id, version } => serialize(crate::type_registry::get_type(
                connection,
                &type_id,
                version.as_deref(),
            )),
            Self::ListVersions(type_id) => {
                let Some(canonical) = crate::type_registry::resolve_type_id(connection, &type_id)
                    .map_err(DbError::Storage)?
                else {
                    return Ok(serde_json::json!([]));
                };
                serialize(crate::type_registry::list_type_versions(
                    connection, &canonical,
                ))
            }
            Self::ResolveAlias(alias) => {
                serialize(crate::type_registry::resolve_alias(connection, &alias))
            }
            Self::ListObjectTypes => serialize(ark_store::object_types::list(connection)),
            Self::GetObjectType(id) => serialize(ark_store::object_types::get(connection, &id)),
            Self::ListObjectLinks => serialize(ark_store::objects::list_object_links(connection)),
        }
    }
}
