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
) -> Result<Vec<crate::canonical_types::pending::ReplayNotice>, String> {
    ark_store::type_registry::persist_type_version(conn, input, created_at)?;
    crate::canonical_types::pending::replay_pending_for_type(conn, &input.type_id, &input.version)
}
