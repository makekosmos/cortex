#![allow(clippy::unwrap_used)]

use crate::committed_feed::{receive_inbox_payload, InboxState};
use crate::test_support::TestDatabase;
use crate::writer::{SqliteWriter, StoreTransactionFailure};
use ark_application::object_write::{NewObjectReceipt, ReceiptOutcome};
use ark_application::sync_storage::{InboundApplyOutcome, InboxReceiptWrite};
use ark_application::task_object_write::{AuthenticatedCoreTaskWrite, RemoteCoreTaskOutcome};
use ark_domain::types::SyncEntity;
use rusqlite::Connection;
use serde_json::{json, Map};

fn database() -> (TestDatabase, Connection) {
    let database = TestDatabase::create().expect("test database initializes");
    let connection = database.open_connection().expect("test database opens");
    connection
        .execute(
            "INSERT INTO device_registry_projection(
                 device_id, signing_public_key, key_epoch, admission_epoch
             ) VALUES ('device-a', zeroblob(32), 1, 1)",
            [],
        )
        .expect("peer is admitted");
    (database, connection)
}

fn object_entity(id: &str, title: &str, hlc: &str) -> SyncEntity {
    SyncEntity {
        entity_type: "object".into(),
        id: id.into(),
        data: serde_json::from_value::<Map<String, serde_json::Value>>(json!({
            "typeId": "core.note",
            "typeVersion": "0.1.0",
            "title": title,
            "contentJson": {"type": "doc", "content": []},
            "propsJson": {"description":null,"extensions":{}},
            "createdAt": "2026-10-15T00:00:00.000Z",
            "updatedAt": "2026-10-15T00:00:00.000Z",
            "deletedAt": null
        }))
        .expect("task payload is an object"),
        hlc: hlc.into(),
        deleted: None,
        origin_device_id: Some("device-a".into()),
        origin_seq: Some(1),
    }
}

fn authenticated_task_write(entity: &SyncEntity, message_id: &str) -> AuthenticatedCoreTaskWrite {
    AuthenticatedCoreTaskWrite::new(
        entity.id.clone(),
        entity.hlc.clone(),
        entity.origin_device_id.clone(),
        entity.data.clone(),
        message_id.into(),
        serde_json::to_string(entity).expect("task entity serializes"),
    )
}

fn core_task_entity(id: &str, title: &str, hlc: &str) -> SyncEntity {
    SyncEntity {
        entity_type: "object".into(),
        id: id.into(),
        data: serde_json::from_value(json!({
            "typeId": "core.task",
            "typeVersion": "0.1.0",
            "title": title,
            "contentJson": {"type": "doc", "content": []},
            "propsJson": {
                "state": {"kind": "open", "at": null},
                "planning": {"kind": "inbox", "period": "any", "date": null},
                "deadline": null,
                "reminders": [],
                "recurrence": null,
                "priority": "none",
                "checklist": [],
                "extensions": {}
            },
            "createdAt": "2026-10-15T00:00:00.000Z",
            "updatedAt": "2026-10-15T00:00:00.000Z",
            "deletedAt": null
        }))
        .expect("task payload is an object"),
        hlc: hlc.into(),
        deleted: None,
        origin_device_id: Some("device-a".into()),
        origin_seq: Some(1),
    }
}

fn write(entity: &SyncEntity, message_id: &str) -> InboxReceiptWrite {
    InboxReceiptWrite::new(
        "device-a".into(),
        message_id.into(),
        serde_json::to_string(entity).expect("entity serializes"),
    )
}

fn count(connection: &Connection, sql: &str, id: &str) -> i64 {
    connection
        .query_row(sql, [id], |row| row.get(0))
        .expect("persisted row count reads")
}

#[path = "sync_transaction_contract_tests/core_task_write.rs"]
mod core_task_write;
#[path = "sync_transaction_contract_tests/inbound_apply.rs"]
mod inbound_apply;
#[path = "sync_transaction_contract_tests/inbox_recovery.rs"]
mod inbox_recovery;
