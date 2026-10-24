#![allow(clippy::unwrap_used)]

use ark_domain::selective_sync::{filter_entities_for_profile, SelectiveSyncProfile, SyncMode};
use ark_domain::types::VersionVector;
use rusqlite::{Connection, OpenFlags};
use serde_json::json;

use crate::test_support::TestDatabase;

#[test]
fn deleted_object_tombstone_keeps_type_through_collection_filter_and_apply() {
    let source_db = TestDatabase::create().unwrap();
    let source_conn = source_db.open_connection().unwrap();
    source_conn
        .execute(
            concat!(
                "INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) ",
                "VALUES('deleted-note','core.note','1.0.0','Note','t','t')"
            ),
            [],
        )
        .unwrap();
    let type_id = crate::objects::get_object(&source_conn, "deleted-note")
        .unwrap()
        .unwrap()
        .type_id;
    crate::objects::delete_object(&source_conn, "deleted-note").unwrap();
    crate::sync_tombstones::record(
        &source_conn,
        "object",
        "deleted-note",
        Some(&type_id),
        "2026-01-02T00:00:00.000Z:000001:source",
    )
    .unwrap();

    let collected = crate::sync_entities::collect_entities(
        &source_conn,
        &VersionVector::default(),
        "source-device",
    )
    .unwrap();
    let mut profile = SelectiveSyncProfile::default();
    profile.set_rule("type", "core.note", SyncMode::Full);
    let tombstone = filter_entities_for_profile(&profile, &collected)
        .into_iter()
        .find(|entity| entity.id == "deleted-note" && entity.deleted == Some(true))
        .expect("typed tombstone should survive collection and profile filtering");
    assert_eq!(tombstone.data["typeId"], json!("core.note"));

    let destination_db = TestDatabase::create().unwrap();
    let mut destination_conn = destination_db.open_connection().unwrap();
    destination_conn
        .execute(
            concat!(
                "INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) ",
                "VALUES('deleted-note','core.note','1.0.0','Note','t','t')"
            ),
            [],
        )
        .unwrap();
    let transaction = destination_conn.transaction().unwrap();
    crate::sync_entities::apply_inbound_entity(&transaction, &tombstone).unwrap();
    transaction.commit().unwrap();

    let guard =
        Connection::open_with_flags(destination_db.path(), OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    assert!(crate::objects::get_object(&guard, "deleted-note")
        .unwrap()
        .is_none());
}
