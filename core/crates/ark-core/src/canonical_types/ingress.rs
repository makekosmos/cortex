//! Engine-owned canonical object write ingress.
use crate::canonical_types::definitions::canonical_type_registrations;
use crate::canonical_types::validation::{validate_canonical, CanonicalValidationCode};
use crate::type_registry;
use crate::types::{ArkObject, ArkObjectWrite};
pub use ark_domain::canonical_type::CanonicalIngressError;
use ark_domain::canonical_type::TypeRegistration;
use rusqlite::Connection;

/// Resolve and validate a new object before any persistence or sync mutation.
/// Non-canonical/package registrations retain the existing compatibility path.
pub fn prepare_object(
    conn: &Connection,
    input: ArkObjectWrite,
) -> Result<ArkObject, CanonicalIngressError> {
    let input_type_id = input.type_id.clone();
    // KOS-397: after the rewrite, com.kosmos.task 1.1.0 is only read, never written.
    if input_type_id == crate::canonical_types::core_task_rewrite::LEGACY_TASK_TYPE {
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

    prepare_resolved_object(input, &canonical_type_id, &resolved_version, registration)
}

pub(crate) fn prepare_resolved_object(
    input: ArkObjectWrite,
    canonical_type_id: &str,
    resolved_version: &str,
    registration: Option<&TypeRegistration>,
) -> Result<ArkObject, CanonicalIngressError> {
    let input_type_id = input.type_id.clone();
    if registration.is_some_and(|registration| {
        registration.type_id != canonical_type_id || registration.version != resolved_version
    }) {
        return Err(CanonicalIngressError::invariant(
            "resolved_registration_mismatch",
        ));
    }
    if input_type_id == crate::canonical_types::core_task_rewrite::LEGACY_TASK_TYPE {
        return Err(CanonicalIngressError::invalid("legacy_task_type_retired"));
    }
    if let Some(registration) = registration {
        if input_type_id != canonical_type_id {
            return Err(CanonicalIngressError::invalid("legacy_alias_new_write"));
        }
        if input.type_version.as_deref() != Some(registration.version.as_str()) {
            return Err(CanonicalIngressError::invalid("canonical_version_required"));
        }
        validate_canonical(registration, &input.props_json, &input.content_json).map_err(
            |error| CanonicalIngressError {
                category: match error.code {
                    CanonicalValidationCode::InvalidField => "invalid_request",
                    CanonicalValidationCode::InvariantViolation => "definition_invariant",
                },
                code: match error.code {
                    CanonicalValidationCode::InvalidField => "canonical_field",
                    CanonicalValidationCode::InvariantViolation => "canonical_definition",
                },
                pointer: Some(error.pointer),
            },
        )?;
    }

    let mut object = input.with_type_version(resolved_version.to_owned());
    object.type_id = canonical_type_id.to_owned();
    Ok(object)
}
