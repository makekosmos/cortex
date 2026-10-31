use super::*;
use serde_json::Value;

/// Return the current first-party registrations.
pub fn canonical_type_registrations() -> Result<Vec<TypeRegistration>, String> {
    let mut registrations: Vec<_> = [
        CORE_TASK_DEFINITION,
        NOTE_DEFINITION,
        PROJECT_DEFINITION,
        TAG_DEFINITION,
        PERSON_DEFINITION,
        IMAGE_DEFINITION,
        TIME_ENTRY_DEFINITION,
        GAME_DEFINITION,
        BOOK_DEFINITION,
    ]
    .into_iter()
    .map(registration_from_literal)
    .collect::<Result<_, _>>()?;
    let task_index = registrations
        .iter()
        .position(|registration| registration.type_id == "core.task")
        .ok_or_else(|| "core.task is not registered".to_owned())?;
    let mut previous_task = registrations[task_index].clone();
    previous_task.version = "0.1.0".into();
    let mut relations: Value =
        serde_json::from_str(&previous_task.relations_json).map_err(|error| error.to_string())?;
    let relations = relations
        .as_array_mut()
        .ok_or_else(|| "core.task relations must be an array".to_owned())?;
    for (relation, target) in relations
        .iter_mut()
        .zip(["com.kosmos.project", "com.kosmos.tag"])
    {
        let target_type = relation
            .get("targetTypeId")
            .and_then(Value::as_str)
            .ok_or_else(|| "core.task relation target is missing".to_owned())?;
        if !matches!(target_type, "core.project" | "core.tag") {
            return Err("unexpected current core.task relation target".into());
        }
        relation["targetTypeId"] = Value::String(target.into());
    }
    previous_task.relations_json =
        serde_json::to_string(&relations).map_err(|error| error.to_string())?;
    let schema: Value =
        serde_json::from_str(&previous_task.schema_json).map_err(|error| error.to_string())?;
    let ui_schema: Value =
        serde_json::from_str(&previous_task.ui_schema_json).map_err(|error| error.to_string())?;
    let content_contract: Value = serde_json::from_str(&previous_task.content_contract_json)
        .map_err(|error| error.to_string())?;
    let relations: Value =
        serde_json::from_str(&previous_task.relations_json).map_err(|error| error.to_string())?;
    let sync_policy: Value =
        serde_json::from_str(&previous_task.sync_policy_json).map_err(|error| error.to_string())?;
    previous_task.schema_hash = canonical_schema_hash(
        &schema,
        &ui_schema,
        &content_contract,
        &relations,
        &sync_policy,
    )?;
    if previous_task.schema_hash
        != "20fbcf5ec311b37563d2b18bf29fb79c781b3eaa9fb28cca76620678f1f8abb1"
    {
        return Err("frozen core.task 0.1.0 hash mismatch".into());
    }
    registrations.insert(task_index, previous_task);
    Ok(registrations)
}

/// Newest registered version of a canonical type. `None` for unknown types.
pub fn current_canonical_version(type_id: &str) -> Result<Option<String>, String> {
    Ok(canonical_type_registrations()?
        .iter()
        .filter(|r| r.type_id == type_id)
        .filter_map(|r| semver::Version::parse(&r.version).ok())
        .max()
        .map(|v| v.to_string()))
}

/// Whether `(type_id, version)` is a live canonical registration. Readers of
/// stored objects must accept every registered version — objects keep the
/// version they were written with.
pub fn is_canonical_version(type_id: &str, version: &str) -> bool {
    canonical_type_registrations()
        .map(|registrations| {
            registrations
                .iter()
                .any(|r| r.type_id == type_id && r.version == version)
        })
        .unwrap_or(false)
}

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
