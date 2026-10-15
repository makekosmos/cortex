use super::count;
use ark_core::db::{clear_all, init_schema};
use rusqlite::{params, Connection};

#[test]
fn clear_all_removes_ephemeral_rows_but_keeps_locked_registry_and_archive() {
    let conn = Connection::open_in_memory().unwrap();
    init_schema(&conn).unwrap();
    conn.execute_batch(
        "INSERT INTO objects(id,type_id,type_version,title,content_json,props_json,created_at,
         updated_at) VALUES('obj-marker','com.kosmos.note','1.0.0','marker','{}','{}','now',
         'now');INSERT INTO object_links(id,source_object_id,target_object_id,link_type,created_at)
         VALUES('link-marker','obj-marker','obj-marker','marker','now');INSERT INTO
         object_local_state(object_id,device_id,data_json,updated_at) VALUES('obj-marker','dev',
         'local-marker','now');INSERT INTO object_sync_versions(object_id,hlc,deleted)
         VALUES('obj-marker','marker-hlc',0);INSERT INTO object_migration_quarantine(object_id,
         contract_version,source_type_id,fields_json,source_hash,updated_at) VALUES('obj-marker',
         'phase3-canonical-v1','com.kosmos.note','quarantine-marker','hash','now')",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO canonical_migration_items(contract_version,source_kind,source_id,source_hash,
         raw_source,status,updated_at) VALUES(?1,?2,?3,?4,?5,'unchanged','now')",
        params!["phase3-canonical-v1", "marker", "marker", "hash", b"marker"],
    )
    .unwrap();
    conn.execute_batch(
        "INSERT INTO usage_sync_heads(device_id,max_seq) VALUES('device-marker',9);
         INSERT INTO integration_configurations(integration_id,provider,account_subject,hlc)
         VALUES('integration-marker','provider','account','marker-hlc');
         INSERT INTO authorized_nodes(node_id,key_fingerprint,signing_public_key,
         encryption_public_key,grant_epoch,status,authorized_at,hlc)
         VALUES('node-marker','fingerprint','signing-key','encryption-key',1,'active','now',
         'marker-hlc');",
    )
    .unwrap();
    let locked_before = count(&conn, "object_types");
    let archive_before = count(&conn, "legacy_type_definition_archive");
    clear_all(&conn).unwrap();
    for table in [
        "objects",
        "object_links",
        "object_local_state",
        "object_sync_versions",
        "object_migration_quarantine",
        "canonical_migration_items",
        "canonical_migration_runs",
    ] {
        assert_eq!(count(&conn, table), 0, "{table} not cleared");
    }
    assert_eq!(count(&conn, "object_types"), locked_before);
    assert_eq!(
        count(&conn, "legacy_type_definition_archive"),
        archive_before
    );
    assert_eq!(count(&conn, "usage_sync_heads"), 1);
    assert_eq!(count(&conn, "integration_configurations"), 1);
    assert_eq!(count(&conn, "authorized_nodes"), 1);
}
