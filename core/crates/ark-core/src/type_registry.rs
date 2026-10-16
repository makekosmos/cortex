mod builtins;
mod registration_migrations;
mod type_catalog;
mod type_updates;
mod version_storage;

pub use builtins::ensure_builtin_versions;
pub use registration_migrations::{migrate_phase2, register_type};
pub use type_catalog::{
    ensure_legacy_type_version, list_aliases, list_all_type_versions, list_type_summaries,
    list_type_versions, register_alias, resolve_object_type_identity, resolve_object_type_version,
};
pub use type_updates::{
    get_type, resolve_alias, resolve_type_id, set_base_type, set_current_version, set_status,
};
pub use version_storage::{
    canonical_schema_hash, insert_type_version, legacy_compatibility_version, AliasRecord,
    TypeGetResponse, TypeRegistration, TypeSummary, TypeVersion, LEGACY_VERSION,
};
