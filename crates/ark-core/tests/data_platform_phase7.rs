#![allow(clippy::unwrap_used)]
use ark_core::{data_platform, db};
use rusqlite::Connection;

#[test]
fn external_refs_are_additive_and_unique() {
    let conn = Connection::open_in_memory().unwrap();
    db::init_schema(&conn).unwrap();
    conn.execute("INSERT INTO objects(id,type_id,type_version,title,created_at,updated_at) VALUES('note-1','com.kosmos.note','1.0.0','Note','now','now')",[]).unwrap();
    data_platform::upsert_external_ref(
        &conn,
        "obsidian",
        "vault",
        "markdown",
        "notes/a.md",
        "note-1",
        Some("1"),
        Some(&"a".repeat(64)),
        "clean",
    )
    .unwrap();
    assert_eq!(
        conn.query_row("SELECT object_id FROM external_refs", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "note-1"
    );
    assert!(data_platform::upsert_external_ref(
        &conn, "", "vault", "markdown", "x", "note-1", None, None, "clean"
    )
    .is_err());
}
