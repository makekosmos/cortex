use ark_domain::canonical_type::registry::{
    canonicalize_type_version, validate_type_id, validate_version,
};
use ark_domain::canonical_type::{TypeRegistration, TypeVersion};
use rusqlite::{params, Connection, OptionalExtension};

pub fn persist_type_version(
    conn: &Connection,
    input: &TypeVersion,
    created_at: &str,
) -> Result<(), String> {
    validate_type_id(&input.type_id, "type id")?;
    validate_version(&input.version)?;
    let canonical = canonicalize_type_version(input)?;
    if !input.schema_hash.is_empty() && input.schema_hash != canonical.schema_hash {
        return Err("schema hash mismatch".into());
    }
    let canonical_exists = conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![input.type_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if canonical_exists.is_none() {
        return Err("canonical type missing".into());
    }
    let existing = conn
        .query_row(
            concat!(
                "SELECT schema_json,ui_schema_json,content_contract_json,relations_json,",
                "sync_policy_json,schema_hash,created_at FROM object_type_versions WHERE ",
                "type_id=?1 AND version=?2"
            ),
            params![input.type_id, input.version],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some((schema, ui_schema, content_contract, relations, sync_policy, hash, _)) = existing {
        if schema == canonical.schema_json
            && ui_schema == canonical.ui_schema_json
            && content_contract == canonical.content_contract_json
            && relations == canonical.relations_json
            && sync_policy == canonical.sync_policy_json
            && hash == canonical.schema_hash
        {
            return Ok(());
        }
        return Err("immutable type version conflict".into());
    }
    conn.execute(
        concat!(
            "INSERT INTO object_type_versions (type_id,version,schema_json,",
            "ui_schema_json,content_contract_json,relations_json,sync_policy_json,",
            "schema_hash,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"
        ),
        params![
            input.type_id,
            input.version,
            canonical.schema_json,
            canonical.ui_schema_json,
            canonical.content_contract_json,
            canonical.relations_json,
            canonical.sync_policy_json,
            canonical.schema_hash,
            created_at
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn insert_type_version(
    conn: &Connection,
    input: &TypeVersion,
    created_at: &str,
) -> Result<Vec<ark_domain::sync_storage::ReplayNotice>, String> {
    persist_type_version(conn, input, created_at)?;
    crate::pending_objects::replay_pending_for_type(conn, &input.type_id, &input.version)
}

/// What the caller means for the `object_types` row of an existing type.
///
/// The two writers of definitions (the phase-3 migration installing builtin
/// types, and `types.register` adding an application type) differ only here.
pub(crate) enum TypeRowWrite {
    /// A new type: insert the row, leave an existing one untouched.
    InsertIfMissing { system_locked: bool },
    /// A builtin over a row that is already canonical: only lock it.
    LockExisting,
    /// Promote a generated legacy row: overwrite every definition column.
    OverwriteExisting { system_locked: bool },
    /// A newer version of an application type: refresh the stored schema.
    RefreshSchema,
}

pub(crate) enum DefinitionWriteError {
    /// A different definition is already stored under this (type, version).
    ImmutableVersionConflict,
    Storage(String),
}

impl DefinitionWriteError {
    fn storage(error: impl std::fmt::Display) -> Self {
        Self::Storage(error.to_string())
    }
}

/// The single writer for a type definition: the `object_types` row, the
/// `object_type_versions` row, and the `current_version` bump.
///
/// Callers keep their own policy around it: the migration archives legacy rows
/// and locks builtin types, `types.register` validates the application
/// namespace and additive compatibility, and both then land here.
pub(crate) fn write_definition(
    conn: &Connection,
    registration: &TypeRegistration,
    type_row: TypeRowWrite,
) -> Result<(), DefinitionWriteError> {
    validate_type_id(&registration.type_id, "type id").map_err(DefinitionWriteError::storage)?;
    validate_version(&registration.version).map_err(DefinitionWriteError::storage)?;
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
    })
    .map_err(DefinitionWriteError::storage)?;

    let exists: Option<String> = conn
        .query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            params![registration.type_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(DefinitionWriteError::storage)?;
    match (&type_row, exists) {
        (TypeRowWrite::InsertIfMissing { system_locked }, None) => {
            conn.execute(
                concat!(
                    "INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,",
                    "updated_at,system_locked,owner_kind,owner_id,current_version,status,",
                    "base_type_id) VALUES(?1,?2,?3,?4,?5,?5,?6,?7,?8,?9,?10,?11)"
                ),
                params![
                    registration.type_id,
                    registration.name,
                    registration.schema_json,
                    registration.ui_schema_json,
                    registration.created_at,
                    i64::from(*system_locked),
                    registration.owner_kind,
                    registration.owner_id,
                    registration.version,
                    registration.status,
                    registration.base_type_id
                ],
            )
            .map_err(DefinitionWriteError::storage)?;
        }
        (TypeRowWrite::LockExisting, Some(_)) => {
            conn.execute(
                "UPDATE object_types SET system_locked=1 WHERE id=?1",
                params![registration.type_id],
            )
            .map_err(DefinitionWriteError::storage)?;
        }
        (TypeRowWrite::OverwriteExisting { system_locked }, Some(_)) => {
            conn.execute(
                concat!(
                    "UPDATE object_types SET name=?2,schema_json=?3,ui_schema_json=?4,",
                    "created_at=?5,updated_at=?5,system_locked=?6,owner_kind=?7,owner_id=?8,",
                    "current_version=?9,status=?10,base_type_id=?11 WHERE id=?1"
                ),
                params![
                    registration.type_id,
                    registration.name,
                    registration.schema_json,
                    registration.ui_schema_json,
                    registration.created_at,
                    i64::from(*system_locked),
                    registration.owner_kind,
                    registration.owner_id,
                    registration.version,
                    registration.status,
                    registration.base_type_id
                ],
            )
            .map_err(DefinitionWriteError::storage)?;
        }
        (TypeRowWrite::RefreshSchema, Some(_)) => {
            conn.execute(
                "UPDATE object_types SET schema_json=?1, updated_at=?2 WHERE id=?3",
                params![
                    canonical.schema_json,
                    registration.created_at,
                    registration.type_id
                ],
            )
            .map_err(DefinitionWriteError::storage)?;
        }
        (TypeRowWrite::InsertIfMissing { .. }, Some(_)) => {}
        (TypeRowWrite::LockExisting, None)
        | (TypeRowWrite::OverwriteExisting { .. }, None)
        | (TypeRowWrite::RefreshSchema, None) => {
            return Err(DefinitionWriteError::Storage(format!(
                "type '{}' is not installed and cannot be updated",
                registration.type_id
            )))
        }
    }

    let stored: Option<(String, String, String, String, String, String)> = conn
        .query_row(
            concat!(
                "SELECT schema_json,ui_schema_json,content_contract_json,relations_json,",
                "sync_policy_json,schema_hash FROM object_type_versions WHERE type_id=?1 AND ",
                "version=?2"
            ),
            params![registration.type_id, registration.version],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(DefinitionWriteError::storage)?;
    match stored {
        Some(existing) => {
            if existing
                != (
                    canonical.schema_json.clone(),
                    canonical.ui_schema_json.clone(),
                    canonical.content_contract_json.clone(),
                    canonical.relations_json.clone(),
                    canonical.sync_policy_json.clone(),
                    canonical.schema_hash.clone(),
                )
            {
                return Err(DefinitionWriteError::ImmutableVersionConflict);
            }
        }
        None => {
            conn.execute(
                concat!(
                    "INSERT INTO object_type_versions(type_id,version,schema_json,",
                    "ui_schema_json,content_contract_json,relations_json,sync_policy_json,",
                    "schema_hash,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)"
                ),
                params![
                    registration.type_id,
                    registration.version,
                    canonical.schema_json,
                    canonical.ui_schema_json,
                    canonical.content_contract_json,
                    canonical.relations_json,
                    canonical.sync_policy_json,
                    canonical.schema_hash,
                    registration.created_at
                ],
            )
            .map_err(DefinitionWriteError::storage)?;
        }
    }

    // A newer definition supersedes the stored current_version: unversioned
    // reads and version resolution must see the newest contract.
    let current_version: Option<String> = conn
        .query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            params![registration.type_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(DefinitionWriteError::storage)?;
    let next = semver::Version::parse(&registration.version).ok();
    let superseded = match (
        current_version
            .as_deref()
            .and_then(|version| semver::Version::parse(version).ok()),
        next,
    ) {
        (Some(current), Some(next)) => next > current,
        (None, Some(_)) => true,
        _ => false,
    };
    if superseded {
        conn.execute(
            "UPDATE object_types SET current_version=?2 WHERE id=?1",
            params![registration.type_id, registration.version],
        )
        .map_err(DefinitionWriteError::storage)?;
    }
    Ok(())
}
