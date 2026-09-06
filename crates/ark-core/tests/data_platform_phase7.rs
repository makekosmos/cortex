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

#[test]
fn sync_profiles_store_frozen_modes_before_objects_and_cascade_rules() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    data_platform::ensure_schema(&conn).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='objects'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );

    conn.execute(
        "INSERT INTO sync_profiles(profile_id,device_id,revision,name,active)
         VALUES('profile-a','device-a',7,'portable',1)",
        [],
    )
    .unwrap();
    for (kind, id, mode, filter) in [
        ("type", "com.kosmos.note", "full", "{}"),
        ("dataset", "usage", "metadata", "{\"days\":30}"),
        ("blob", "attachments", "none", "{}"),
    ] {
        conn.execute(
            "INSERT INTO sync_profile_rules(profile_id,resource_kind,resource_id,mode,filter_json)
             VALUES(?1,?2,?3,?4,?5)",
            rusqlite::params!["profile-a", kind, id, mode, filter],
        )
        .unwrap();
    }

    let profile: (String, i64, i64) = conn
        .query_row(
            "SELECT device_id,revision,active FROM sync_profiles WHERE profile_id='profile-a'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(profile, ("device-a".into(), 7, 1));
    let modes: Vec<String> = conn
        .prepare(
            "SELECT mode FROM sync_profile_rules
             WHERE profile_id='profile-a' ORDER BY resource_kind",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(modes, ["none", "metadata", "full"]);

    assert!(conn
        .execute(
            "INSERT INTO sync_profile_rules(profile_id,resource_kind,resource_id,mode)
             VALUES('profile-a','type','bad','secret')",
            [],
        )
        .is_err());
    conn.execute("CREATE TABLE objects (id TEXT PRIMARY KEY)", [])
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sync_profiles WHERE profile_id='profile-a'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
    conn.execute("DELETE FROM sync_profiles WHERE profile_id='profile-a'", [])
        .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sync_profile_rules WHERE profile_id='profile-a'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );
}
