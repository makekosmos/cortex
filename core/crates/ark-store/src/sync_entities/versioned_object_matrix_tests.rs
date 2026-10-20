#![allow(clippy::unwrap_used)]

use ark_domain::types::{ArkObject, ObjectType, SyncEntity};
use rusqlite::Connection;
use serde_json::{json, Value};

fn setup_db() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&connection).unwrap();
    connection
}

fn apply_entity(connection: &mut Connection, entity: &SyncEntity) {
    let transaction = connection.transaction().unwrap();
    crate::sync_entities::apply_inbound_entity(&transaction, entity).unwrap();
    transaction.commit().unwrap();
}

fn object_entity(id: &str, type_id: &str, title: &str) -> SyncEntity {
    let object = ArkObject {
        id: id.to_string(),
        type_id: type_id.to_string(),
        type_version: "1.0.0".to_string(),
        title: title.to_string(),
        content_json: json!({"body": title}),
        props_json: json!({}),
        created_at: "2026-01-01T00:00:00.000Z".to_string(),
        updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        deleted_at: None,
    };
    let Value::Object(mut data) = serde_json::to_value(object).unwrap() else {
        panic!("expected an object payload");
    };
    data.remove("id");
    SyncEntity {
        entity_type: "object".to_string(),
        id: id.to_string(),
        data,
        hlc: "2026-01-01T00:00:00.000Z:000010:peer-a".to_string(),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    }
}

#[test]
fn versioned_object_matrix_holds_unknown_payload_and_replays_exactly() {
    let mut connection = setup_db();
    crate::type_registry::upsert_object_type(
        &connection,
        &ObjectType {
            id: "remote-type".to_string(),
            name: "Remote".to_string(),
            schema_json: json!({
                "fields": [{
                    "id": "description",
                    "label": "Описание",
                    "kind": "long_text",
                    "required": false,
                    "visible": true,
                    "read_only": false,
                }]
            })
            .to_string(),
            ui_schema_json: json!({
                "visible_fields": ["description"],
                "hidden_fields": ["created_at", "updated_at", "deleted_at"],
                "read_only_fields": [],
            })
            .to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            system_locked: false,
        },
    )
    .unwrap();

    let mut old = object_entity("remote-old", "remote-type", "old-wire");
    old.data.remove("typeVersion");
    apply_entity(&mut connection, &old);
    assert_eq!(
        crate::objects::get_object(&connection, "remote-old")
            .unwrap()
            .unwrap()
            .type_version,
        ark_domain::canonical_type::registry::LEGACY_VERSION
    );

    let mut unknown = object_entity("remote-unknown", "remote-type", "unknown-wire");
    unknown.data.insert("typeVersion".into(), json!("9.9.9"));
    unknown
        .data
        .insert("futurePayload".into(), json!({"keep": true}));
    apply_entity(&mut connection, &unknown);

    assert!(crate::objects::get_object(&connection, "remote-unknown")
        .unwrap()
        .is_none());
    let payload: String = connection
        .query_row(
            "SELECT payload FROM sync_pending_objects WHERE id='remote-unknown'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(payload.contains("futurePayload"));
    let feed_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id='remote-unknown'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(feed_rows, 0, "pending data is not yet a committed change");

    let replayed = crate::pending_objects::replay_pending_for_type(
        &connection,
        "remote-type",
        ark_domain::canonical_type::registry::LEGACY_VERSION,
    )
    .unwrap();
    assert!(replayed.is_empty());
    assert!(crate::objects::get_object(&connection, "remote-unknown")
        .unwrap()
        .is_none());
}
