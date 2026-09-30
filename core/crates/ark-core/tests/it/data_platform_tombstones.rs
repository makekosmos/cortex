#![allow(clippy::unwrap_used)]

use ark_core::db::SqliteStorageBackend;
use ark_core::sync_server::StorageBackend;
use ark_core::{data_platform, db};
use rusqlite::Connection;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn deleted_object_tombstone_keeps_type_through_collection_filter_and_apply() {
    let source_conn = Connection::open_in_memory().unwrap();
    db::init_schema(&source_conn).unwrap();
    source_conn
        .execute(
            "INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) VALUES('deleted-note','com.kosmos.note','1.0.0','Note','t','t')",
            [],
        )
        .unwrap();
    let type_id = db::get_object(&source_conn, "deleted-note")
        .unwrap()
        .unwrap()
        .type_id;
    db::delete_object(&source_conn, "deleted-note").unwrap();
    db::record_sync_tombstone_with_type(
        &source_conn,
        "object",
        "deleted-note",
        Some(&type_id),
        "2026-01-02T00:00:00.000Z:000001:source",
    )
    .unwrap();

    let source = Arc::new(SqliteStorageBackend::new(Arc::new(Mutex::new(source_conn))));
    let mut profile = data_platform::SelectiveSyncProfile::default();
    profile.set_rule("type", "com.kosmos.note", data_platform::SyncMode::Full);
    source.set_selective_sync_profile(Some(profile));
    let loaded = source.load_entities(&HashMap::new()).await;
    let tombstone = loaded
        .iter()
        .find(|entity| entity.id == "deleted-note" && entity.deleted == Some(true))
        .cloned()
        .expect("typed tombstone should survive collection and profile filtering");
    assert_eq!(tombstone.data["typeId"], json!("com.kosmos.note"));

    let destination_conn = Connection::open_in_memory().unwrap();
    db::init_schema(&destination_conn).unwrap();
    destination_conn
        .execute(
            "INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) VALUES('deleted-note','com.kosmos.note','1.0.0','Note','t','t')",
            [],
        )
        .unwrap();
    let destination_db = Arc::new(Mutex::new(destination_conn));
    let destination = SqliteStorageBackend::new(destination_db.clone());
    destination.apply_entity(&tombstone).await.unwrap();
    let guard = destination_db.lock().unwrap();
    assert!(db::get_object(&guard, "deleted-note").unwrap().is_none());
}
