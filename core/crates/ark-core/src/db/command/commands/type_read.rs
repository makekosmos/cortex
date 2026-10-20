use super::super::{sealed, CommandClass, CommandContext, DbError, StoreReadQuery};

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

impl StoreReadQuery for TypeReadQuery {
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
        capabilities: &ark_store::ReadCapabilities<'_>,
    ) -> Result<Self::Output, DbError> {
        context.require_engine()?;
        match self {
            Self::ListSummaries => serialize(capabilities.type_registry().list_summaries()),
            Self::Get { type_id, version } => serialize(
                capabilities
                    .type_registry()
                    .get(&type_id, version.as_deref()),
            ),
            Self::ListVersions(type_id) => {
                serialize(capabilities.type_registry().list_versions(&type_id))
            }
            Self::ResolveAlias(alias) => {
                serialize(capabilities.type_registry().resolve_alias(&alias))
            }
            Self::ListObjectTypes => serialize(capabilities.type_registry().list_object_types()),
            Self::GetObjectType(id) => serialize(capabilities.type_registry().get_object_type(&id)),
            Self::ListObjectLinks => serialize(capabilities.type_registry().list_object_links()),
        }
    }
}
