#![allow(clippy::unwrap_used)]
use ark_application::sync_storage::{SyncEntityMutationPort, SyncEntityReadPort};

use super::SqliteStorageBackend;
use ark_store::test_support::TestDatabase;
use rusqlite::{Connection, OpenFlags};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn deleted_object_tombstone_keeps_type_through_collection_filter_and_apply() {
    let source_db = TestDatabase::create().unwrap();
    let source_conn = source_db.open_connection().unwrap();
    ark_store::open::init_schema(&source_conn).unwrap();
    source_conn
        .execute(
            concat!(
                "INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) ",
                "VALUES('deleted-note','com.kosmos.note','1.0.0','Note','t','t')"
            ),
            [],
        )
        .unwrap();
    let type_id = ark_store::objects::get_object(&source_conn, "deleted-note")
        .unwrap()
        .unwrap()
        .type_id;
    ark_store::objects::delete_object(&source_conn, "deleted-note").unwrap();
    ark_store::sync_tombstones::record(
        &source_conn,
        "object",
        "deleted-note",
        Some(&type_id),
        "2026-01-02T00:00:00.000Z:000001:source",
    )
    .unwrap();

    drop(source_conn);
    let source = Arc::new(
        SqliteStorageBackend::open_for_test(
            source_db.path().to_string_lossy().into_owned(),
            "source-device",
        )
        .await
        .unwrap(),
    );
    let mut profile = ark_domain::selective_sync::SelectiveSyncProfile::default();
    profile.set_rule(
        "type",
        "com.kosmos.note",
        ark_domain::selective_sync::SyncMode::Full,
    );
    source.set_selective_sync_profile(Some(profile));
    let loaded = source.load_entities(&HashMap::new()).await.unwrap();
    let tombstone = loaded
        .iter()
        .find(|entity| entity.id == "deleted-note" && entity.deleted == Some(true))
        .cloned()
        .expect("typed tombstone should survive collection and profile filtering");
    assert_eq!(tombstone.data["typeId"], json!("com.kosmos.note"));

    let destination_db = TestDatabase::create().unwrap();
    let destination_conn = destination_db.open_connection().unwrap();
    ark_store::open::init_schema(&destination_conn).unwrap();
    destination_conn
        .execute(
            concat!(
                "INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) ",
                "VALUES('deleted-note','com.kosmos.note','1.0.0','Note','t','t')"
            ),
            [],
        )
        .unwrap();
    drop(destination_conn);
    let destination = SqliteStorageBackend::open_for_test(
        destination_db.path().to_string_lossy().into_owned(),
        "destination-device",
    )
    .await
    .unwrap();
    destination.apply_entity(&tombstone).await.unwrap();
    let guard =
        Connection::open_with_flags(destination_db.path(), OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    assert!(ark_store::objects::get_object(&guard, "deleted-note")
        .unwrap()
        .is_none());
}
