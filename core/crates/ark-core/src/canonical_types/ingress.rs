//! Engine-owned canonical object write ingress.
use crate::canonical_types::definitions::canonical_type_registrations;
use crate::type_registry;
use crate::types::{ArkObject, ArkObjectWrite};
pub use ark_domain::canonical_type::CanonicalIngressError;
use rusqlite::Connection;

/// Resolve and validate a new object before any persistence or sync mutation.
/// Non-canonical/package registrations retain the existing compatibility path.
pub fn prepare_object(
    conn: &Connection,
    input: ArkObjectWrite,
) -> Result<ArkObject, CanonicalIngressError> {
    let input_type_id = input.type_id.clone();
    // KOS-397: after the rewrite, com.kosmos.task 1.1.0 is only read, never written.
    if input_type_id == ark_domain::canonical_type::LEGACY_TASK_TYPE {
        return Err(CanonicalIngressError::invalid("legacy_task_type_retired"));
    }
    let requested_version = input.type_version.clone();
    let (canonical_type_id, resolved_version) = type_registry::resolve_object_type_identity(
        conn,
        &input_type_id,
        requested_version.as_deref(),
    )
    .map_err(|_| CanonicalIngressError::invalid("unknown_type_or_version"))?;

    let registrations = canonical_type_registrations()
        .map_err(|_| CanonicalIngressError::invariant("canonical_registry"))?;
    let registration = registrations.iter().find(|registration| {
        registration.type_id == canonical_type_id && registration.version == resolved_version
    });

    ark_domain::canonical_type::prepare_resolved_object(
        input,
        &canonical_type_id,
        &resolved_version,
        registration,
    )
}
