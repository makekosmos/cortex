
pub fn resolve_alias(conn: &Connection, alias: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT canonical_type_id FROM object_type_aliases WHERE alias=?1",
        params![alias],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn resolve_type_id(conn: &Connection, input: &str) -> Result<Option<String>, String> {
    if conn
        .query_row(
            "SELECT 1 FROM object_types WHERE id=?1",
            params![input],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Ok(Some(input.to_string()));
    }
    resolve_alias(conn, input)
}

pub fn get_type(
    conn: &Connection,
    input: &str,
    requested_version: Option<&str>,
) -> Result<Option<TypeGetResponse>, String> {
    let Some(type_id) = resolve_type_id(conn, input)? else {
        return Ok(None);
    };
    let summary = list_type_summaries(conn)?
        .into_iter()
        .find(|s| s.type_id == type_id);
    let Some(summary) = summary else {
        return Ok(None);
    };
    let version = requested_version.unwrap_or(&summary.current_version);
    let definition = list_type_versions(conn, &type_id)?
        .into_iter()
        .find(|v| v.version == version);
    Ok(definition.map(|definition| TypeGetResponse {
        summary,
        definition,
    }))
}
pub fn set_current_version(conn: &Connection, type_id: &str, version: &str) -> Result<(), String> {
    if version != LEGACY_VERSION {
        validate_version(version)?;
    }
    let exists = conn
        .query_row(
            "SELECT 1 FROM object_type_versions WHERE type_id=?1 AND version=?2",
            params![type_id, version],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        return Err("current version definition missing".into());
    }
    if conn
        .execute(
            "UPDATE object_types SET current_version=?1 WHERE id=?2",
            params![version, type_id],
        )
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("canonical type missing".into());
    }
    Ok(())
}

pub fn set_status(conn: &Connection, type_id: &str, status: &str) -> Result<(), String> {
    if !matches!(status, "active" | "deprecated" | "pending") {
        return Err("invalid status".into());
    }
    if conn
        .execute(
            "UPDATE object_types SET status=?1 WHERE id=?2",
            params![status, type_id],
        )
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("canonical type missing".into());
    }
    Ok(())
}

pub fn set_base_type(conn: &Connection, type_id: &str, base: Option<&str>) -> Result<(), String> {
    if base == Some(type_id) {
        return Err("base self-reference".into());
    }
    if let Some(base) = base {
        if conn
            .query_row(
                "SELECT 1 FROM object_types WHERE id=?1",
                params![base],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err("base type missing".into());
        }
        let mut cursor = Some(base.to_string());
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id.clone()) {
                return Err("base type cycle".into());
            }
            if id == type_id {
                return Err("base type cycle".into());
            }
            cursor = conn
                .query_row(
                    "SELECT base_type_id FROM object_types WHERE id=?1",
                    params![id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .flatten();
        }
    }
    if conn
        .execute(
            "UPDATE object_types SET base_type_id=?1 WHERE id=?2",
            params![base, type_id],
        )
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("canonical type missing".into());
    }
    Ok(())
}
