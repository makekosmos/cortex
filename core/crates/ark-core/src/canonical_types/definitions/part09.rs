fn registration_from_literal(raw: &str) -> Result<TypeRegistration, String> {
    let definition: Value = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    let object = definition
        .as_object()
        .ok_or_else(|| "canonical definition must be a JSON object".to_owned())?;
    let string = |field: &str| {
        object
            .get(field)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("canonical definition field {field} must be a string"))
    };
    let schema = object.get("schema").ok_or("missing schema")?;
    let ui_schema = object.get("uiSchema").ok_or("missing uiSchema")?;
    let content_contract = object
        .get("contentContract")
        .ok_or("missing contentContract")?;
    let relations = object.get("relations").ok_or("missing relations")?;
    let sync_policy = object.get("syncPolicy").ok_or("missing syncPolicy")?;
    let expected_hash = string("schemaHash")?;
    let actual_hash =
        canonical_schema_hash(schema, ui_schema, content_contract, relations, sync_policy)?;
    if actual_hash != expected_hash {
        return Err(format!(
            "frozen canonical hash mismatch for {}: expected {expected_hash}, got {actual_hash}",
            string("typeId")?
        ));
    }
    let aliases = object
        .get("aliases")
        .and_then(Value::as_array)
        .ok_or("canonical definition aliases must be an array")?
        .iter()
        .map(|alias| {
            let alias = alias
                .as_str()
                .ok_or("canonical definition alias must be a string")?;
            Ok(AliasRecord {
                alias: alias.to_owned(),
                canonical_type_id: string("typeId")?,
                created_at: string("createdAt")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(TypeRegistration {
        type_id: string("typeId")?,
        name: string("name")?,
        schema_json: serde_json::to_string(schema).map_err(|error| error.to_string())?,
        ui_schema_json: serde_json::to_string(ui_schema).map_err(|error| error.to_string())?,
        content_contract_json: serde_json::to_string(content_contract)
            .map_err(|error| error.to_string())?,
        relations_json: serde_json::to_string(relations).map_err(|error| error.to_string())?,
        sync_policy_json: serde_json::to_string(sync_policy).map_err(|error| error.to_string())?,
        version: string("version")?,
        schema_hash: expected_hash,
        owner_kind: string("ownerKind")?,
        owner_id: object
            .get("ownerId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        status: string("status")?,
        base_type_id: object
            .get("baseTypeId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        aliases,
        created_at: string("createdAt")?,
    })
}

/// Version of the day-field contract fix: day-granularity props moved from
/// `format: "date-time"` to `format: "date"`.
const DAY_CONTRACT_VERSION: &str = "1.1.0";

/// Derive a newer registration from an existing definition without copying its
/// literal: set `version`, flip the listed property paths to `format: "date"`,
/// recompute `schemaHash`, then run the result through the same frozen-hash
/// self-check as hand-written literals.
fn evolved_registration(
    base: &str,
    version: &str,
    date_properties: &[&[&str]],
) -> Result<TypeRegistration, String> {
    let mut definition: Value = serde_json::from_str(base).map_err(|e| e.to_string())?;
    *definition
        .get_mut("version")
        .ok_or("canonical definition missing version")? = serde_json::json!(version);
    for path in date_properties {
        let mut node = definition
            .get_mut("schema")
            .and_then(Value::as_object_mut)
            .ok_or("canonical definition missing schema")?;
        for segment in *path {
            node = node
                .get_mut("properties")
                .and_then(|p| p.get_mut(segment))
                .and_then(Value::as_object_mut)
                .ok_or_else(|| format!("canonical definition missing property {segment}"))?;
        }
        node.insert("format".into(), serde_json::json!("date"));
    }
    let field = |name: &str| {
        definition
            .get(name)
            .cloned()
            .ok_or_else(|| format!("canonical definition missing {name}"))
    };
    let hash = canonical_schema_hash(
        &field("schema")?,
        &field("uiSchema")?,
        &field("contentContract")?,
        &field("relations")?,
        &field("syncPolicy")?,
    )?;
    *definition
        .get_mut("schemaHash")
        .ok_or("canonical definition missing schemaHash")? = Value::String(hash);
    registration_from_literal(&serde_json::to_string(&definition).map_err(|e| e.to_string())?)
}

