use super::{CORE_TASK_ID, CORE_TASK_VERSION};
use ark_domain::object_write::RemoteObjectWrite;
use ark_domain::types::ObjectLink;
use serde_json::{Map, Value};

#[derive(Debug, PartialEq, Eq)]
pub enum RemoteCoreTaskWriteDecodeError {
    IncompleteRepresentation,
    UnsupportedType,
    InvalidObject(String),
}

pub fn decode_remote_core_task_write(
    id: String,
    source_hlc: String,
    source_origin_device_id: Option<String>,
    data: Map<String, Value>,
) -> Result<RemoteObjectWrite, RemoteCoreTaskWriteDecodeError> {
    for key in [
        "typeId",
        "typeVersion",
        "title",
        "createdAt",
        "updatedAt",
        "deletedAt",
        "contentJson",
        "propsJson",
    ] {
        if !data.contains_key(key) {
            return Err(RemoteCoreTaskWriteDecodeError::IncompleteRepresentation);
        }
    }
    if data.get("typeId").and_then(Value::as_str) != Some(CORE_TASK_ID)
        || data.get("typeVersion").and_then(Value::as_str) != Some(CORE_TASK_VERSION)
    {
        return Err(RemoteCoreTaskWriteDecodeError::UnsupportedType);
    }
    let mut data = data;
    data.insert("id".into(), Value::String(id));
    let links: Vec<ObjectLink> = data
        .remove("links")
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| RemoteCoreTaskWriteDecodeError::InvalidObject(error.to_string()))?
        .unwrap_or_default();
    let object = serde_json::from_value(Value::Object(data))
        .map_err(|error| RemoteCoreTaskWriteDecodeError::InvalidObject(error.to_string()))?;
    Ok(RemoteObjectWrite {
        object,
        links,
        source_hlc,
        source_origin_device_id,
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_remote_core_task_write, RemoteCoreTaskWriteDecodeError};
    use serde_json::{json, Map, Value};

    fn task_data() -> Map<String, Value> {
        json!({
            "typeId": "core.task",
            "typeVersion": "0.1.0",
            "title": "Task",
            "createdAt": "created",
            "updatedAt": "updated",
            "deletedAt": null,
            "contentJson": {},
            "propsJson": {},
            "links": []
        })
        .as_object()
        .expect("task fixture is a JSON object")
        .clone()
    }

    #[test]
    fn remote_task_decoder_requires_full_representation() {
        let mut data = task_data();
        data.remove("deletedAt");
        assert!(matches!(
            decode_remote_core_task_write("id".into(), "hlc".into(), None, data),
            Err(RemoteCoreTaskWriteDecodeError::IncompleteRepresentation)
        ));
    }

    #[test]
    fn remote_task_decoder_rejects_other_types() {
        let mut data = task_data();
        data.insert("typeId".into(), Value::String("note".into()));
        assert!(matches!(
            decode_remote_core_task_write("id".into(), "hlc".into(), None, data),
            Err(RemoteCoreTaskWriteDecodeError::UnsupportedType)
        ));
    }

    #[test]
    fn remote_task_decoder_preserves_identity_and_links() {
        let remote = decode_remote_core_task_write(
            "task-id".into(),
            "hlc".into(),
            Some("origin".into()),
            task_data(),
        )
        .expect("valid core.task full representation decodes");
        assert_eq!(remote.object.id, "task-id");
        assert_eq!(remote.object.type_id, "core.task");
        assert!(remote.links.is_empty());
        assert_eq!(remote.source_hlc, "hlc");
        assert_eq!(remote.source_origin_device_id.as_deref(), Some("origin"));
    }
}
