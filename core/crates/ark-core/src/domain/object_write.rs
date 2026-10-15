#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectWriteSnapshot {
    pub exists: bool,
    pub type_id: Option<String>,
    pub type_version: Option<String>,
    pub revision: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RemoteObjectWrite {
    pub object: crate::types::ArkObjectWrite,
    pub links: Vec<crate::types::ObjectLink>,
    pub source_hlc: String,
    pub source_origin_device_id: Option<String>,
}
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Serialize)]
struct CanonicalObjectPayload<'a> {
    object: &'a crate::types::ArkObject,
    links: &'a [crate::types::ObjectLink],
}

pub(crate) fn canonical_payload_digest(
    object: &crate::types::ArkObject,
    links: &[crate::types::ObjectLink],
) -> Result<(String, String), String> {
    let value = serde_json::to_value(CanonicalObjectPayload { object, links })
        .map_err(|error| error.to_string())?;
    let payload_json =
        serde_json::to_string(&canonical_value(&value)).map_err(|error| error.to_string())?;
    let payload_sha256 = format!("{:x}", Sha256::digest(payload_json.as_bytes()));
    Ok((payload_json, payload_sha256))
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted = object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect::<Map<_, _>>())
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_value).collect()),
        other => other.clone(),
    }
}
