use crate::types::SyncEntity;
use serde_json::Value;

pub(super) fn is_full_core_task(entity: &SyncEntity) -> bool {
    entity.entity_type == "object"
        && entity.deleted != Some(true)
        && entity.data.get("typeId").and_then(Value::as_str) == Some("core.task")
        && entity.data.get("typeVersion").and_then(Value::as_str) == Some("0.1.0")
        && entity.data.contains_key("contentJson")
        && entity.data.contains_key("propsJson")
}
