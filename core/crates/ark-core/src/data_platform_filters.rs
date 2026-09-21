pub fn validate_filter_json(input: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(input).map_err(|_| "invalid filter JSON".to_string())?;
    let value = if value == Value::Object(Default::default()) {
        serde_json::json!({"version": 1, "where": {"and": []}})
    } else {
        value
    };
    let object = value
        .as_object()
        .ok_or_else(|| "filter must be an object".to_string())?;
    if object.len() != 2
        || object.get("version") != Some(&Value::from(1))
        || !object.contains_key("where")
    {
        return Err("invalid FilterV1 envelope".into());
    }
    let mut predicates = 0;
    validate_predicate(&object["where"], 1, &mut predicates)?;
    if predicates > MAX_PREDICATES {
        return Err("too many predicates".into());
    }
    Ok(canonical_json(&value))
}

pub fn filter_digest(input: &str) -> Result<[u8; 32], String> {
    let canonical = validate_filter_json(input)?;
    Ok(Sha256::digest(canonical.as_bytes()).into())
}

fn validate_predicate(value: &Value, depth: usize, count: &mut usize) -> Result<(), String> {
    if depth > MAX_FILTER_DEPTH {
        return Err("filter depth exceeded".into());
    }
    let object = value
        .as_object()
        .ok_or_else(|| "predicate must be an object".to_string())?;
    if object.len() != 1 {
        return Err("predicate must have one operator".into());
    }
    let (operator, operand) = object.iter().next().expect("predicate object is non-empty");
    *count += 1;
    match operator.as_str() {
        "and" | "or" => {
            let items = operand
                .as_array()
                .ok_or_else(|| "predicate list required".to_string())?;
            if (items.is_empty() && !(depth == 1 && operator == "and"))
                || items.len() > MAX_PREDICATES
            {
                return Err("predicate list must contain 1..128 items".into());
            }
            for item in items {
                validate_predicate(item, depth + 1, count)?;
            }
        }
        "not" => validate_predicate(operand, depth + 1, count)?,
        "eq" => validate_comparison(operand, false)?,
        "in" => validate_comparison(operand, true)?,
        "exists" => {
            let object = operand
                .as_object()
                .ok_or_else(|| "exists operand required".to_string())?;
            if !has_exact_keys(object, &["field", "value"])
                || !object["field"].is_string()
                || !object["value"].is_boolean()
            {
                return Err("invalid exists predicate".into());
            }
        }
        _ => return Err("unknown filter operator".into()),
    }
    Ok(())
}

fn validate_comparison(value: &Value, many: bool) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "comparison operand required".to_string())?;
    let keys = if many {
        &["field", "values"][..]
    } else {
        &["field", "value"][..]
    };
    if !has_exact_keys(object, keys) || !object["field"].is_string() {
        return Err("invalid comparison predicate".into());
    }
    if many {
        let values = object["values"]
            .as_array()
            .ok_or_else(|| "in.values must be an array".to_string())?;
        if values.is_empty()
            || values.len() > MAX_PREDICATES
            || values.iter().any(|v| !is_scalar(v))
        {
            return Err("invalid in.values".into());
        }
    } else if !is_scalar(&object["value"]) {
        return Err("invalid comparison value".into());
    }
    Ok(())
}

fn is_scalar(value: &Value) -> bool {
    value.is_null() || value.is_boolean() || value.is_number() || value.is_string()
}

fn has_exact_keys(object: &serde_json::Map<String, Value>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|key| format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("JSON object key is serializable"),
                        canonical_json(&map[key])
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => serde_json::to_string(value).expect("JSON value is serializable"),
    }
}
