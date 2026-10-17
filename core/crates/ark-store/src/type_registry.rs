mod builtins;
mod catalog;
mod resolution;
mod updates;
mod version_storage;

pub use builtins::ensure_builtin_versions;
pub use catalog::{
    ensure_legacy_type_version, list_aliases, list_all_type_versions, list_type_summaries,
    list_type_versions, register_alias, resolve_object_type_identity, resolve_object_type_version,
};
pub use resolution::{resolve_alias, resolve_type_id};
pub use updates::{get_type, set_base_type, set_current_version, set_status};
pub use version_storage::persist_type_version;
