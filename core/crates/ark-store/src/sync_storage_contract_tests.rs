#![allow(clippy::unwrap_used)]

use ark_domain::hlc::HLC;
use ark_domain::types::VersionVector;
use rusqlite::Connection;

use crate::{open, sync_kv, sync_tombstones, sync_versions, VERSION_VECTOR_KEY};

fn setup_db() -> Connection {
    let connection = Connection::open_in_memory().expect("database opens");
    open::init_schema(&connection).expect("schema initializes");
    connection
}

#[test]
fn local_sync_version_bump_persists_hlc_for_entity() {
    let connection = setup_db();
    let first = sync_versions::bump_sync_version_vector(
        &connection,
        "object",
        "obj-local",
        "device-local",
        false,
    )
    .unwrap();
    let second = sync_versions::bump_sync_version_vector(
        &connection,
        "object",
        "obj-local",
        "device-local",
        false,
    )
    .unwrap();

    assert!(HLC::is_newer(&second, &first) || second > first);
    let raw = sync_kv::get_sync_kv(&connection, VERSION_VECTOR_KEY)
        .unwrap()
        .expect("version vector should be stored");
    let vector: VersionVector = serde_json::from_str(&raw).unwrap();
    assert_eq!(vector.get("obj-local"), Some(&second));
    assert!(second.ends_with(":device-local"));
}

#[test]
fn local_sync_tombstone_is_persisted_and_can_be_cleared() {
    let connection = setup_db();
    sync_tombstones::record(
        &connection,
        "object",
        "obj-deleted",
        None,
        "2026-01-02T00:00:00.000Z:000001:device-local",
    )
    .unwrap();

    let tombstones = sync_tombstones::load_all(&connection).unwrap();
    assert!(
        tombstones.iter().any(|entity| {
            entity.entity_type == "object"
                && entity.id == "obj-deleted"
                && entity.deleted == Some(true)
        }),
        "local delete should leave a durable tombstone",
    );

    sync_tombstones::delete(&connection, "obj-deleted").unwrap();
    let tombstones = sync_tombstones::load_all(&connection).unwrap();
    assert!(
        tombstones.iter().all(|entity| entity.id != "obj-deleted"),
        "local live upsert should be able to clear prior tombstone",
    );
}
