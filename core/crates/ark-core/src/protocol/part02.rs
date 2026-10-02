#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_hello_serialization() {
        let msg = LanSyncMessage::Hello {
            protocol_version: 1,
            device_id: "dev-1".to_string(),
            device_name: "MacBook".to_string(),
            space_id: "abc123".to_string(),
            addresses: Some(vec!["192.168.1.70:21531".to_string()]),
            auth_nonce: None,
            auth_hmac: None,
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "hello");
        assert_eq!(parsed["protocol_version"], 1);
        assert_eq!(parsed["device_id"], "dev-1");
        assert_eq!(parsed["device_name"], "MacBook");
        assert_eq!(parsed["space_id"], "abc123");
    }

    #[test]
    fn test_version_vector_serialization() {
        let mut vector = crate::types::VersionVector::new();
        vector.insert(
            "e1".to_string(),
            "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
        );

        let msg = LanSyncMessage::VersionVector {
            vector,
            origin_device_id: None,
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "version_vector");
        assert!(parsed["vector"]["e1"].is_string());
    }

    #[test]
    fn test_sync_changes_serialization() {
        let entity = SyncEntity {
            entity_type: "todo".to_string(),
            id: "t1".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };
        let msg = LanSyncMessage::SyncChanges {
            batch_id: "123-abc".to_string(),
            entities: vec![entity],
            is_last: true,
            origin_device_id: None,
            usage_complete_through: None,
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "sync_changes");
        assert_eq!(parsed["batch_id"], "123-abc");
        assert_eq!(parsed["is_last"], true);
        assert_eq!(parsed["entities"][0]["type"], "todo");
        // deleted should be omitted when None
        assert!(parsed["entities"][0].get("deleted").is_none());
    }

    #[test]
    fn test_sync_changes_usage_complete_through_compat() {
        // Old senders omit the field — a new receiver deserializes to None
        // and keeps contiguous-only cursor advancement (KOS-302).
        let json = r#"{"type":"sync_changes","batch_id":"b1","entities":[],"is_last":true}"#;
        let msg = deserialize_message(json).unwrap();
        match msg {
            LanSyncMessage::SyncChanges {
                usage_complete_through,
                ..
            } => assert!(usage_complete_through.is_none()),
            other => panic!("expected SyncChanges, got {other:?}"),
        }

        // A new sender's claim round-trips to a new receiver.
        let mut through = std::collections::HashMap::new();
        through.insert("dev-a".to_string(), 42_u64);
        let msg = LanSyncMessage::SyncChanges {
            batch_id: "b2".to_string(),
            entities: vec![],
            is_last: true,
            origin_device_id: None,
            usage_complete_through: Some(through),
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&serialize_message(&msg)).unwrap();
        assert_eq!(parsed["usage_complete_through"]["dev-a"], 42);
    }

    #[test]
    fn test_sync_entity_deleted_field() {
        let entity = SyncEntity {
            entity_type: "todo".to_string(),
            id: "t1".to_string(),
            data: serde_json::Map::new(),
            hlc: "2026-01-01T00:00:00.000Z:000000:dev".to_string(),
            deleted: Some(true),
            origin_device_id: None,
            origin_seq: None,
        };
        let json_str = serde_json::to_string(&entity).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["deleted"], true);
        assert_eq!(parsed["type"], "todo");
    }

    #[test]
    fn test_live_change_serialization() {
        let entity = SyncEntity {
            entity_type: "project".to_string(),
            id: "p1".to_string(),
            data: {
                let mut m = serde_json::Map::new();
                m.insert("title".to_string(), json!("My Project"));
                m
            },
            hlc: "2026-01-01T00:00:00.000Z:000001:dev".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };
        let msg = LanSyncMessage::LiveChange {
            change_id: "ch-1".to_string(),
            entity,
            origin_device_id: None,
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "live_change");
        assert_eq!(parsed["change_id"], "ch-1");
        assert_eq!(parsed["entity"]["type"], "project");
    }

    #[test]
    fn test_ping_pong_serialization() {
        let ping = LanSyncMessage::Ping { ts: 1234567890 };
        let json_str = serialize_message(&ping);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "ping");
        assert_eq!(parsed["ts"], 1234567890u64);

        let pong = LanSyncMessage::Pong { ts: 1234567890 };
        let json_str = serialize_message(&pong);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "pong");
    }

    #[test]
    fn test_peer_list_serialization() {
        let msg = LanSyncMessage::PeerList {
            peers: vec![PeerRecord {
                device_id: "d1".to_string(),
                device_name: "Phone".to_string(),
                addresses: vec!["10.0.0.1:21531".to_string()],
                last_seen: "2026-01-01T00:00:00.000Z".to_string(),
                last_address: Some("10.0.0.1:21531".to_string()),
            }],
        };
        let json_str = serialize_message(&msg);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["type"], "peer_list");
        assert_eq!(parsed["peers"][0]["device_id"], "d1");
    }

    #[test]
    fn test_message_roundtrip() {
        let msg = LanSyncMessage::SyncAck {
            batch_id: "b1".to_string(),
            accepted: 42,
        };
        let json_str = serialize_message(&msg);
        let deserialized = deserialize_message(&json_str).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn signed_integration_messages_roundtrip() {
        let frame = crate::integration_replication::SignedSyncEnvelope::new(
            "space",
            "origin",
            "recipient",
            1,
            "message",
            Vec::new(),
            "signature",
        );
        for message in [
            LanSyncMessage::SignedIntegrationFrame { frame },
            LanSyncMessage::SignedIntegrationAck {
                message_id: "message".into(),
                accepted: true,
            },
        ] {
            let serialized = serialize_message(&message);
            assert_eq!(deserialize_message(&serialized), Some(message));
        }
    }

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        assert!(id.contains('-'));
        let parts: Vec<&str> = id.splitn(2, '-').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].parse::<u128>().is_ok());
        assert_eq!(parts[1].len(), 6);
    }

    #[test]
    fn test_deserialize_ts_compatible_hello() {
        // JSON exactly as the TS implementation would produce
        let json = concat!(r#"{"type":"hello","protocol_version":1,"device_id":"dev-123","#,r#""device_name":"Electron","space_id":"abcdef0123456789","#,r#""addresses":["192.168.1.70:21531"]}"#);
        let msg = deserialize_message(json).unwrap();
        match msg {
            LanSyncMessage::Hello {
                protocol_version,
                device_id,
                device_name,
                space_id,
                addresses,
                auth_nonce,
                auth_hmac,
            } => {
                assert_eq!(protocol_version, 1);
                assert_eq!(device_id, "dev-123");
                assert_eq!(device_name, "Electron");
                assert_eq!(space_id, "abcdef0123456789");
                assert_eq!(addresses, Some(vec!["192.168.1.70:21531".to_string()]));
                assert_eq!(auth_nonce, None);
                assert_eq!(auth_hmac, None);
            }
            other => panic!("expected Hello, got {other:?}"),
        }
    }

    #[test]
    fn test_deserialize_ts_compatible_sync_entity() {
        let json = concat!(r#"{"type":"todo","id":"550e8400-e29b-41d4-a716-446655440000","#,r#""data":{"title":"Buy milk","isCompleted":false,"tagIds":["tag1"]},"#,r#""hlc":"2026-03-28T14:30:00.123Z:000042:delphi-web-abc123"}"#);
        let entity: SyncEntity = serde_json::from_str(json).unwrap();
        assert_eq!(entity.entity_type, "todo");
        assert_eq!(entity.id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(entity.data["title"], "Buy milk");
        assert!(entity.deleted.is_none());
    }
}
