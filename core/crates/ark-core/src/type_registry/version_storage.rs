pub use crate::domain::canonical_type::{
    canonical_schema_hash, AliasRecord, TypeRegistration, TypeSummary, TypeVersion,
};
pub use ark_application::type_registry::TypeGetResponse;
use ark_domain::canonical_type::registry::{
    canonicalize_type_version, validate_type_id, validate_version,
};
use rusqlite::{params, Connection, OptionalExtension};

pub use ark_domain::canonical_type::registry::{legacy_compatibility_version, LEGACY_VERSION};

pub fn insert_type_version(
    conn: &Connection,
    input: &TypeVersion,
    created_at: &str,
) -> Result<Vec<crate::canonical_types::pending::ReplayNotice>, String> {
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
        .map_err(|e| e.to_string())?;
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
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some((a, b, c, d, e, f, _)) = existing {
        if a == canonical.schema_json
            && b == canonical.ui_schema_json
            && c == canonical.content_contract_json
            && d == canonical.relations_json
            && e == canonical.sync_policy_json
            && f == canonical.schema_hash
        {
            return crate::canonical_types::pending::replay_pending_for_type(
                conn,
                &input.type_id,
                &input.version,
            );
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
    .map_err(|e| e.to_string())?;
    crate::canonical_types::pending::replay_pending_for_type(conn, &input.type_id, &input.version)
}
