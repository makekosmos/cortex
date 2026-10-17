pub mod entities;
pub mod objects;
pub mod sync;
pub mod usage;

pub use entities::*;
pub use objects::*;
pub use sync::*;
pub use usage::*;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ArkObjectWrite, PeerRecord, SyncEntity};

    #[test]
    fn object_write_keeps_legacy_default_json_shape() {
        let object: ArkObjectWrite = serde_json::from_value(json!({
            "id": "object-1",
            "typeId": "note",
            "title": "Example",
            "createdAt": "2026-10-17T00:00:00Z",
            "updatedAt": "2026-10-17T00:00:00Z",
            "deletedAt": null
        }))
        .unwrap();

        assert_eq!(object.type_version, None);
        assert_eq!(
            object.content_json,
            json!({"type": "doc", "content": [{"type": "paragraph"}]})
        );
        assert_eq!(object.props_json, json!({}));
    }

    #[test]
    fn sync_contracts_keep_their_wire_field_names() {
        let entity = SyncEntity {
            entity_type: "todo".into(),
            id: "todo-1".into(),
            data: serde_json::Map::new(),
            hlc: "1:0:device".into(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };
        let peer = PeerRecord {
            device_id: "device-1".into(),
            device_name: "Laptop".into(),
            addresses: vec![],
            last_seen: "now".into(),
            last_address: None,
            platform: None,
            app_version: None,
        };

        assert_eq!(
            serde_json::to_value(entity).unwrap(),
            json!({"type": "todo", "id": "todo-1", "data": {}, "hlc": "1:0:device"})
        );
        assert_eq!(
            serde_json::to_value(peer).unwrap(),
            json!({
                "device_id": "device-1",
                "device_name": "Laptop",
                "addresses": [],
                "last_seen": "now"
            })
        );
    }
}
