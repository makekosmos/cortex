pub use ark_application::type_registry::TypeGetResponse;
pub use ark_domain::canonical_type::{
    canonical_schema_hash, AliasRecord, TypeRegistration, TypeSummary, TypeVersion,
};
use rusqlite::Connection;

pub use ark_domain::canonical_type::registry::{legacy_compatibility_version, LEGACY_VERSION};

pub fn insert_type_version(
    conn: &Connection,
    input: &TypeVersion,
    created_at: &str,
) -> Result<Vec<ark_application::sync_storage::ReplayNotice>, String> {
    ark_store::type_registry::persist_type_version(conn, input, created_at)?;
    ark_store::pending_objects::replay_pending_for_type(conn, &input.type_id, &input.version)
}
