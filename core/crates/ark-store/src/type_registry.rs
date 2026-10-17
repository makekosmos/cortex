mod catalog;
mod resolution;

pub use catalog::{
    ensure_legacy_type_version, list_aliases, list_all_type_versions, list_type_summaries,
    list_type_versions, register_alias, resolve_object_type_identity, resolve_object_type_version,
};
pub use resolution::{resolve_alias, resolve_type_id};
