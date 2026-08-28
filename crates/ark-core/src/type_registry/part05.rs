
pub fn ensure_builtin_versions(conn: &Connection) -> Result<(), String> {
    let mut stmt=conn.prepare("SELECT id,schema_json,ui_schema_json,created_at FROM object_types WHERE NOT EXISTS (SELECT 1 FROM object_type_versions v WHERE v.type_id=object_types.id AND v.version=?1)").map_err(|e|e.to_string())?;
    let rows = stmt
        .query_map(params![LEGACY_VERSION], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    for (id, schema, ui, created) in rows {
        insert_legacy_type_version(conn, &id, &schema, &ui, &created)?;
    }
    Ok(())
}

fn insert_legacy_type_version(
    conn: &Connection,
    type_id: &str,
    schema: &str,
    ui_schema: &str,
    created_at: &str,
) -> Result<(), String> {
    let schema_value = parse_json(schema, "schema_json")?;
    let ui_value = parse_json(ui_schema, "ui_schema_json")?;
    let schema_json =
        serde_json::to_string(&canonical_value(&schema_value)).map_err(|e| e.to_string())?;
    let ui_schema_json =
        serde_json::to_string(&canonical_value(&ui_value)).map_err(|e| e.to_string())?;
    let hash = canonical_schema_hash(
        &schema_value,
        &ui_value,
        &json_empty(),
        &json_empty_array(),
        &json_empty(),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES (?1,?2,?3,?4,'{}','[]','{}',?5,?6)",
        params![type_id, LEGACY_VERSION, schema_json, ui_schema_json, hash, created_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
