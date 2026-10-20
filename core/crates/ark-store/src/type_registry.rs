mod builtins;
mod catalog;
mod legacy_mutations;
mod migrations;
mod query_ids;
mod registration;
mod resolution;
mod updates;
mod version_storage;

pub use builtins::ensure_builtin_versions;
pub use catalog::{
    ensure_legacy_type_version, insert_type_record, list_aliases, list_all_type_versions,
    list_type_summaries, list_type_versions, register_alias, resolve_object_type_identity,
    resolve_object_type_version, type_name,
};
pub use legacy_mutations::{delete_object_type, upsert_object_type};
pub use migrations::migrate_schema;
pub use query_ids::stored_type_ids_for_query;
pub use registration::register_type;
pub use resolution::{resolve_alias, resolve_type_id};
pub use updates::{get_type, set_base_type, set_current_version, set_status};
pub use version_storage::persist_type_version;
