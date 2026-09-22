
pub fn ensure_legacy_type_version(
    conn: &Connection,
    type_id: &str,
    schema_json: &str,
    ui_schema_json: &str,
    created_at: &str,
) -> Result<(), String> {
    let current_version: Option<String> = conn
        .query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            params![type_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if current_version
        .as_deref()
        .is_some_and(|version| version != LEGACY_VERSION)
    {
        return Ok(());
    }
    let has_versions: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='object_type_versions')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if !has_versions {
        return Ok(());
    }
    let schema = parse_json(schema_json, "schema_json")?;
    let ui_schema = parse_json(ui_schema_json, "ui_schema_json")?;
    let empty = serde_json::json!({});
    let relations = serde_json::json!([]);
    let hash = canonical_schema_hash(&schema, &ui_schema, &empty, &relations, &empty)?;
    conn.execute(
        "INSERT OR IGNORE INTO object_type_versions (type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            type_id,
            LEGACY_VERSION,
            schema_json,
            ui_schema_json,
            "{}",
            "[]",
            "{}",
            hash,
            created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn resolve_object_type_identity(
    conn: &Connection,
    input_type_id: &str,
    requested: Option<&str>,
) -> Result<(String, String), String> {
    let canonical_type_id = resolve_type_id(conn, input_type_id)?
        .ok_or_else(|| format!("unknown object type '{input_type_id}'"))?;
    let current: String = conn
        .query_row(
            "SELECT current_version FROM object_types WHERE id=?1",
            params![canonical_type_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    // Legacy aliases were removed as registry authorities. Their historical
    // version is accepted at the boundary but must never be persisted.
    let version = match requested {
        None => current.clone(),
        Some(version) if input_type_id != canonical_type_id && version == LEGACY_VERSION => {
            current.clone()
        }
        Some(version) => version.to_string(),
    };
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM object_type_versions WHERE type_id=?1 AND version=?2)",
            params![canonical_type_id, version],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())?
        != 0;
    if !exists {
        return Err(format!(
            "unknown object type version '{canonical_type_id}@{version}'"
        ));
    }
    Ok((canonical_type_id, version))
}

pub fn resolve_object_type_version(
    conn: &Connection,
    type_id: &str,
    requested: Option<&str>,
) -> Result<String, String> {
    resolve_object_type_identity(conn, type_id, requested).map(|(_, version)| version)
}
pub fn list_type_summaries(conn: &Connection) -> Result<Vec<TypeSummary>, String> {
    let mut stmt = conn.prepare("SELECT id,owner_kind,owner_id,current_version,status,base_type_id FROM object_types ORDER BY id ASC").map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([], |r| {
            Ok(TypeSummary {
                type_id: r.get(0)?,
                owner_kind: r.get(1)?,
                owner_id: r.get(2)?,
                current_version: r.get(3)?,
                status: r.get(4)?,
                base_type_id: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}

pub fn list_all_type_versions(conn: &Connection) -> Result<Vec<TypeVersion>, String> {
    let mut values = Vec::new();
    let mut stmt = conn.prepare("SELECT type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at FROM object_type_versions ORDER BY type_id,version").map_err(|e| e.to_string())?;
    for row in stmt
        .query_map([], |r| {
            Ok(TypeVersion {
                type_id: r.get(0)?,
                version: r.get(1)?,
                schema_json: r.get(2)?,
                ui_schema_json: r.get(3)?,
                content_contract_json: r.get(4)?,
                relations_json: r.get(5)?,
                sync_policy_json: r.get(6)?,
                schema_hash: r.get(7)?,
                created_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
    {
        values.push(row.map_err(|e| e.to_string())?);
    }
    values.sort_by(|a, b| {
        a.type_id
            .cmp(&b.type_id)
            .then_with(|| {
                Version::parse(&a.version)
                    .ok()
                    .cmp(&Version::parse(&b.version).ok())
            })
            .then_with(|| a.version.cmp(&b.version))
    });
    Ok(values)
}

pub fn list_aliases(conn: &Connection) -> Result<Vec<AliasRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT alias,canonical_type_id,created_at FROM object_type_aliases ORDER BY alias",
        )
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([], |r| {
            Ok(AliasRecord {
                alias: r.get(0)?,
                canonical_type_id: r.get(1)?,
                created_at: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    result
}
pub fn list_type_versions(conn: &Connection, type_id: &str) -> Result<Vec<TypeVersion>, String> {
    let mut stmt = conn.prepare("SELECT type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at FROM object_type_versions WHERE type_id=?1").map_err(|e| e.to_string())?;
    let mut values = stmt
        .query_map(params![type_id], |r| {
            Ok(TypeVersion {
                type_id: r.get(0)?,
                version: r.get(1)?,
                schema_json: r.get(2)?,
                ui_schema_json: r.get(3)?,
                content_contract_json: r.get(4)?,
                relations_json: r.get(5)?,
                sync_policy_json: r.get(6)?,
                schema_hash: r.get(7)?,
                created_at: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    values.sort_by(
        |a, b| match (Version::parse(&a.version), Version::parse(&b.version)) {
            (Ok(a_version), Ok(b_version)) => a_version
                .cmp(&b_version)
                .then_with(|| a.version.cmp(&b.version)),
            (Err(_), Err(_)) => a.version.cmp(&b.version),
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        },
    );
    for value in &values {
        if value.version != LEGACY_VERSION {
            validate_version(&value.version)
                .map_err(|error| format!("corrupt stored version: {error}"))?;
        }
    }
    Ok(values)
}

pub fn register_alias(conn: &Connection, alias: &AliasRecord) -> Result<(), String> {
    validate_id(&alias.alias, "alias")?;
    validate_id(&alias.canonical_type_id, "canonical type id")?;
    if alias.alias == alias.canonical_type_id {
        return Err("self alias".into());
    }
    if conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![alias.alias],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err("alias collides with canonical type".into());
    }
    if conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![alias.canonical_type_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err("unknown canonical type".into());
    }
    let existing = conn
        .query_row(
            "SELECT canonical_type_id FROM object_type_aliases WHERE alias=?1",
            params![alias.alias],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(existing) = existing {
        if existing == alias.canonical_type_id {
            return Ok(());
        }
        return Err("alias conflict".into());
    }
    conn.execute(
        "INSERT INTO object_type_aliases(alias,canonical_type_id,created_at) VALUES (?1,?2,?3)",
        params![alias.alias, alias.canonical_type_id, alias.created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
