mod version_storage;

pub use ark_store::type_registry::{
    ensure_builtin_versions, ensure_legacy_type_version, get_type, list_aliases,
    list_all_type_versions, list_type_summaries, list_type_versions, register_alias, resolve_alias,
    resolve_object_type_identity, resolve_object_type_version, resolve_type_id, set_base_type,
    set_current_version, set_status,
};
pub use version_storage::{
    canonical_schema_hash, insert_type_version, legacy_compatibility_version, AliasRecord,
    TypeGetResponse, TypeRegistration, TypeSummary, TypeVersion, LEGACY_VERSION,
};
