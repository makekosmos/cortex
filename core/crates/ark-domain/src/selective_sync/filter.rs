use serde_json::Value;
use sha2::{Digest, Sha256};

const MAX_FILTER_DEPTH: usize = 16;
const MAX_PREDICATES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterV1 {
    value: Value,
    canonical_json: String,
}

impl FilterV1 {
    pub fn parse(input: &str) -> Result<Self, String> {
        let canonical_json = validate_filter_json(input)?;
        let value = serde_json::from_str(&canonical_json)
            .map_err(|_| "canonical filter could not be decoded".to_string())?;
        Ok(Self {
            value,
            canonical_json,
        })
    }

    pub fn match_all() -> Self {
        Self {
            value: serde_json::json!({"version": 1, "where": {"and": []}}),
            canonical_json: r#"{"version":1,"where":{"and":[]}}"#.into(),
        }
    }

    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }

    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.canonical_json.as_bytes()).into()
    }

    /// Filter fields name exact, case-sensitive keys in the entity's public data map.
    pub fn matches(&self, fields: &serde_json::Map<String, Value>) -> bool {
        evaluate_predicate(&self.value["where"], fields)
    }
}

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
    Ok(FilterV1::parse(input)?.digest())
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

fn evaluate_predicate(value: &Value, fields: &serde_json::Map<String, Value>) -> bool {
    let Some((operator, operand)) = value.as_object().and_then(|object| object.iter().next())
    else {
        return false;
    };
    match operator.as_str() {
        "and" => operand
            .as_array()
            .is_some_and(|items| items.iter().all(|item| evaluate_predicate(item, fields))),
        "or" => operand
            .as_array()
            .is_some_and(|items| items.iter().any(|item| evaluate_predicate(item, fields))),
        "not" => !evaluate_predicate(operand, fields),
        "eq" => compare_field(operand, fields, "value", |actual, expected| {
            actual == expected
        }),
        "in" => compare_field(operand, fields, "values", |actual, expected| {
            expected
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value == actual))
        }),
        "exists" => operand
            .get("field")
            .and_then(Value::as_str)
            .zip(operand.get("value").and_then(Value::as_bool))
            .is_some_and(|(field, expected)| fields.contains_key(field) == expected),
        _ => false,
    }
}

fn compare_field(
    operand: &Value,
    fields: &serde_json::Map<String, Value>,
    expected_key: &str,
    compare: impl FnOnce(&Value, &Value) -> bool,
) -> bool {
    operand
        .get("field")
        .and_then(Value::as_str)
        .and_then(|field| fields.get(field))
        .zip(operand.get(expected_key))
        .is_some_and(|(actual, expected)| compare(actual, expected))
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
