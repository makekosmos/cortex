//! Engine-owned canonical object write ingress.
use crate::canonical_types::definitions::canonical_type_registrations;
use crate::canonical_types::validation::{validate_canonical, CanonicalValidationCode};
use crate::type_registry;
use crate::types::{ArkObject, ArkObjectWrite};
use rusqlite::Connection;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalIngressError {
    pub category: &'static str,
    pub code: &'static str,
    pub pointer: Option<String>,
}

impl CanonicalIngressError {
    fn invalid(code: &'static str) -> Self {
        Self {
            category: "invalid_request",
            code,
            pointer: None,
        }
    }

    fn invariant(code: &'static str) -> Self {
        Self {
            category: "definition_invariant",
            code,
            pointer: None,
        }
    }
}

impl std::fmt::Display for CanonicalIngressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "canonical_ingress:{}:{}", self.category, self.code)?;
        if let Some(pointer) = &self.pointer {
            write!(f, ":{pointer}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CanonicalIngressError {}

/// Resolve and validate a new object before any persistence or sync mutation.
/// Non-canonical/package registrations retain the existing compatibility path.
pub fn prepare_object(
    conn: &Connection,
    input: ArkObjectWrite,
) -> Result<ArkObject, CanonicalIngressError> {
    let input_type_id = input.type_id.clone();
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

    if let Some(registration) = registration {
        if input_type_id != canonical_type_id {
            return Err(CanonicalIngressError::invalid("legacy_alias_new_write"));
        }
        if requested_version.as_deref() != Some(registration.version.as_str()) {
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

    let mut object = input.with_type_version(resolved_version);
    object.type_id = canonical_type_id;
    Ok(object)
}
