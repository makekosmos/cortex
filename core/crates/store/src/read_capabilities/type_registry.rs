use super::TypeRegistryReadCapabilities;
use domain::r#type::{TypeSummary, TypeVersion};
use domain::type_registry::TypeGetResponse;
use domain::types::ObjectLink;

impl TypeRegistryReadCapabilities<'_> {
    pub fn list_summaries(&self) -> Result<Vec<TypeSummary>, String> {
        crate::type_registry::list_type_summaries(self.connection)
    }

    pub fn get(
        &self,
        type_id: &str,
        version: Option<&str>,
    ) -> Result<Option<TypeGetResponse>, String> {
        crate::type_registry::get_type(self.connection, type_id, version)
    }

    pub fn list_versions(&self, type_id: &str) -> Result<Vec<TypeVersion>, String> {
        let Some(canonical) = crate::type_registry::resolve_type_id(self.connection, type_id)?
        else {
            return Ok(Vec::new());
        };
        crate::type_registry::list_type_versions(self.connection, &canonical)
    }

    pub fn resolve_alias(&self, alias: &str) -> Result<Option<String>, String> {
        crate::type_registry::resolve_alias(self.connection, alias)
    }

    pub fn list_object_links(&self) -> Result<Vec<ObjectLink>, String> {
        crate::objects::list_object_links(self.connection)
    }
}
