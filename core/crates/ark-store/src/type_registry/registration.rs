use ark_domain::canonical_type::registry::{
    canonicalize_type_version, is_system_type, validate_type_id, validate_type_namespace,
    validate_version,
};
use ark_domain::canonical_type::validation::{
    check_additive_schema_change, validate_type_schema, CanonicalValidationError,
};
use ark_domain::canonical_type::{TypeRegistration, TypeVersion};
use rusqlite::Connection;
use serde_json::Value;

use super::{
    definition_write_message, get_type, register_alias, resolve_alias, type_name,
    version_storage::{write_definition, TypeRowWrite},
};
use crate::pending_objects::{replay_pending_for_type, ReplayNotice};

/// Register a type supplied by an application (not a package archive).
///
/// A new type is inserted through the regular registration path. A new
/// version of an existing type is appended only when it is an additive schema
/// change (`types.register`); this keeps historical versions readable.
pub fn register_application_type(
    connection: &Connection,
    registration: &TypeRegistration,
) -> Result<Vec<ReplayNotice>, String> {
    validate_application_namespace(registration)?;
    let schema: Value = serde_json::from_str(&registration.schema_json)
        .map_err(|error| format!("invalid schema JSON: {error}"))?;
    validate_type_schema(&schema).map_err(schema_validation_message)?;
    if schema.get("type").and_then(Value::as_str) != Some("object") {
        return Err("schema root type must be \"object\"".into());
    }

    let canonical = canonicalize_type_version(&TypeVersion {
        type_id: registration.type_id.clone(),
        version: registration.version.clone(),
        schema_json: registration.schema_json.clone(),
        ui_schema_json: registration.ui_schema_json.clone(),
        content_contract_json: registration.content_contract_json.clone(),
        relations_json: registration.relations_json.clone(),
        sync_policy_json: registration.sync_policy_json.clone(),
        schema_hash: registration.schema_hash.clone(),
        created_at: registration.created_at.clone(),
    })?;
    let normalized = TypeRegistration {
        schema_json: canonical.schema_json.clone(),
        ui_schema_json: canonical.ui_schema_json.clone(),
        content_contract_json: canonical.content_contract_json.clone(),
        relations_json: canonical.relations_json.clone(),
        sync_policy_json: canonical.sync_policy_json.clone(),
        schema_hash: canonical.schema_hash.clone(),
        ..registration.clone()
    };

    let type_id = &normalized.type_id;
    if type_name(connection, type_id)?.is_none() {
        return register_type(connection, &normalized);
    }

    let existing = get_type(connection, type_id, None)?
        .ok_or_else(|| format!("type registry row for '{type_id}' is missing"))?;
    if existing.summary.owner_id.as_deref() != normalized.owner_id.as_deref() {
        return Err(format!(
            "type '{}' is owned by '{}', not '{}'",
            type_id,
            existing.summary.owner_id.as_deref().unwrap_or("system"),
            normalized.owner_id.as_deref().unwrap_or("system"),
        ));
    }

    let candidate_version = validate_version(&normalized.version)?;
    let current_version = validate_version(&existing.summary.current_version)?;
    if candidate_version < current_version {
        return Err(format!(
            "version '{}' is older than the current version '{}' of type '{}'",
            normalized.version, existing.summary.current_version, type_id
        ));
    }
    if candidate_version == current_version {
        if existing.definition.schema_json == normalized.schema_json {
            return Ok(Vec::new());
        }
        return Err(format!(
            "version '{}' of type '{}' already exists with a different schema",
            normalized.version, type_id
        ));
    }

    let previous: Value = serde_json::from_str(&existing.definition.schema_json)
        .map_err(|error| format!("stored schema for type '{type_id}' is invalid: {error}"))?;
    check_additive_schema_change(&previous, &schema)
        .map_err(|reason| format!("schema is not backward compatible: {reason}"))?;

    let now = chrono::Utc::now().to_rfc3339();
    connection
        .execute_batch("SAVEPOINT ark_registry_application_registration")
        .map_err(|error| error.to_string())?;
    let result = (|| {
        let written = TypeRegistration {
            created_at: now.clone(),
            ..normalized.clone()
        };
        write_definition(connection, &written, TypeRowWrite::RefreshSchema)
            .map_err(definition_write_message)?;
        crate::pending_objects::replay_pending_for_type(connection, type_id, &normalized.version)
    })();
    match result {
        Ok(notices) => {
            connection
                .execute_batch("RELEASE SAVEPOINT ark_registry_application_registration")
                .map_err(|error| error.to_string())?;
            Ok(notices)
        }
        Err(error) => {
            let _ = connection.execute_batch(concat!(
                "ROLLBACK TO SAVEPOINT ark_registry_application_registration; ",
                "RELEASE SAVEPOINT ark_registry_application_registration"
            ));
            Err(error)
        }
    }
}

fn validate_application_namespace(registration: &TypeRegistration) -> Result<(), String> {
    if is_system_type(&registration.type_id)? {
        return Err(format!(
            "type '{}' is a system type and cannot be registered",
            registration.type_id
        ));
    }
    if !registration.type_id.starts_with("com.") {
        return Err(format!(
            "type id '{}' must be in a com.<app>.* namespace",
            registration.type_id
        ));
    }
    let owner_id = registration
        .owner_id
        .as_deref()
        .ok_or_else(|| "application type requires an owner id".to_string())?;
    validate_type_namespace(&registration.type_id, owner_id)
}

fn schema_validation_message(error: CanonicalValidationError) -> String {
    let pointer = if error.pointer.is_empty() {
        "/".to_string()
    } else {
        error.pointer
    };
    let keyword = error.keyword.unwrap_or_else(|| "schema".into());
    format!("invalid schema at {pointer}: {keyword}")
}

pub fn register_type(
    connection: &Connection,
    registration: &TypeRegistration,
) -> Result<Vec<ReplayNotice>, String> {
    validate_type_id(&registration.type_id, "type id")?;
    if registration.owner_kind.trim().is_empty() {
        return Err("invalid owner kind".into());
    }
    if registration.name.trim().is_empty() {
        return Err("invalid type name".into());
    }
    validate_version(&registration.version)?;
    if !matches!(
        registration.status.as_str(),
        "active" | "deprecated" | "pending"
    ) {
        return Err("invalid status".into());
    }
    if let Some(base_type_id) = &registration.base_type_id {
        validate_type_id(base_type_id, "base type id")?;
    }
    for alias in &registration.aliases {
        if alias.canonical_type_id != registration.type_id {
            return Err("alias canonical type mismatch".into());
        }
    }
    if let Some(existing_name) = type_name(connection, &registration.type_id)? {
        let existing = get_type(
            connection,
            &registration.type_id,
            Some(&registration.version),
        )?
        .ok_or_else(|| "canonical type already exists".to_string())?;
        let definition = &existing.definition;
        let canonical = canonicalize_type_version(&TypeVersion {
            type_id: registration.type_id.clone(),
            version: registration.version.clone(),
            schema_json: registration.schema_json.clone(),
            ui_schema_json: registration.ui_schema_json.clone(),
            content_contract_json: registration.content_contract_json.clone(),
            relations_json: registration.relations_json.clone(),
            sync_policy_json: registration.sync_policy_json.clone(),
            schema_hash: registration.schema_hash.clone(),
            created_at: registration.created_at.clone(),
        })?;
        if existing_name == registration.name
            && existing.summary.owner_kind == registration.owner_kind
            && existing.summary.owner_id == registration.owner_id
            && existing.summary.status == registration.status
            && existing.summary.base_type_id == registration.base_type_id
            && definition.type_id == registration.type_id
            && definition.version == registration.version
            && definition.schema_json == canonical.schema_json
            && definition.ui_schema_json == canonical.ui_schema_json
            && definition.content_contract_json == canonical.content_contract_json
            && definition.relations_json == canonical.relations_json
            && definition.sync_policy_json == canonical.sync_policy_json
            && definition.schema_hash == canonical.schema_hash
            && definition.created_at == registration.created_at
        {
            return Ok(Vec::new());
        }
        return Err("canonical type already exists".into());
    }

    if resolve_alias(connection, &registration.type_id)?.is_some() {
        return Err("canonical type id collides with alias".into());
    }

    connection
        .execute_batch("SAVEPOINT ark_registry_registration")
        .map_err(|error| error.to_string())?;
    let result = (|| {
        write_definition(
            connection,
            registration,
            TypeRowWrite::InsertIfMissing {
                system_locked: false,
            },
        )
        .map_err(definition_write_message)?;
        let mut notices =
            replay_pending_for_type(connection, &registration.type_id, &registration.version)?;
        for alias in &registration.aliases {
            register_alias(connection, alias)?;
        }
        notices.extend(replay_pending_for_type(
            connection,
            &registration.type_id,
            &registration.version,
        )?);
        Ok(notices)
    })();
    match result {
        Ok(notices) => {
            connection
                .execute_batch("RELEASE SAVEPOINT ark_registry_registration")
                .map_err(|error| error.to_string())?;
            Ok(notices)
        }
        Err(error) => {
            let _ = connection.execute_batch(concat!(
                "ROLLBACK TO SAVEPOINT ark_registry_registration; RELEASE SAVEPOINT ",
                "ark_registry_registration"
            ));
            Err(error)
        }
    }
}
