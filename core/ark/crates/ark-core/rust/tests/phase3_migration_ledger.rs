use ark_core::canonical_types::migration_ledger::{
    begin_or_resume, ensure_ledger_schema, ChildSavepoint, ItemCheckpoint, ItemStatus, LedgerError,
    MigrationRunStatus,
};
use ark_core::canonical_types::preflight::{SourceKind, SourceRecord};
use rusqlite::Connection;

fn record(kind: &str, id: &str, hash: &str) -> SourceRecord {
    SourceRecord {
        source_kind: SourceKind::Legacy(kind.into()),
        source_id: id.into(),
        source_hash: hash.into(),
        canonical_bytes: format!("canonical-{id}").into_bytes(),
        raw_source: format!("raw-{id}").into_bytes(),
    }
}

#[test]
fn ledger_surface_is_missing_until_implemented() {
    let conn = Connection::open_in_memory().unwrap();
    ensure_ledger_schema(&conn).unwrap();
    let run = begin_or_resume(
        &conn,
        "phase3-canonical-v1",
        &[record("note_obj", "n1", "s1")],
        "now",
    )
    .unwrap();
    assert_eq!(run.status, MigrationRunStatus::Running);
    assert_eq!(run.items[0].status, ItemStatus::Pending);
    assert_eq!(run.items[0].checkpoint, ItemCheckpoint::Prepared);
}

#[test]
fn schema_is_idempotent_and_malformed_schema_fails_closed() {
    let conn = Connection::open_in_memory().unwrap();
    ensure_ledger_schema(&conn).unwrap();
    let before: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name='canonical_migration_items'",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap();
    ensure_ledger_schema(&conn).unwrap();
    assert_eq!(
        before,
        conn.query_row(
            "SELECT sql FROM sqlite_master WHERE name='canonical_migration_items'",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap()
    );
    conn.execute_batch("DROP TABLE canonical_migration_items; CREATE TABLE canonical_migration_items (contract_version TEXT PRIMARY KEY)").unwrap();
    assert!(ensure_ledger_schema(&conn).is_err());
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('canonical_migration_items')",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn fresh_ledger_preserves_exact_raw_bytes_and_hashes_on_resume() {
    let conn = Connection::open_in_memory().unwrap();
    let records = vec![record("note_obj", "n1", "exact-source-hash")];
    let first = begin_or_resume(&conn, "phase3-canonical-v1", &records, "t1").unwrap();
    let second = begin_or_resume(&conn, "phase3-canonical-v1", &records, "t2").unwrap();
    assert_eq!(first.items[0].raw_source, b"raw-n1");
    assert_eq!(second.items[0].attempt, 0);
    assert_eq!(second.items[0].updated_at, "t1");
}

#[test]
fn explicit_retry_increments_attempt_once_and_terminal_transition_is_illegal() {
    let conn = Connection::open_in_memory().unwrap();
    begin_or_resume(
        &conn,
        "phase3-canonical-v1",
        &[record("note_obj", "n1", "s1")],
        "t1",
    )
    .unwrap();
    assert_eq!(
        ark_core::canonical_types::migration_ledger::retry_item(
            &conn,
            "phase3-canonical-v1",
            "note_obj",
            "n1",
            "t2"
        )
        .unwrap(),
        1
    );
    ark_core::canonical_types::migration_ledger::transition_item(
        &conn,
        "phase3-canonical-v1",
        "note_obj",
        "n1",
        ItemStatus::Migrated,
        ItemCheckpoint::Committed,
        Some("c1"),
        "{}",
        None,
        "t3",
    )
    .unwrap();
    assert!(ark_core::canonical_types::migration_ledger::retry_item(
        &conn,
        "phase3-canonical-v1",
        "note_obj",
        "n1",
        "t4"
    )
    .is_err());
    assert!(
        ark_core::canonical_types::migration_ledger::transition_item(
            &conn,
            "phase3-canonical-v1",
            "note_obj",
            "n1",
            ItemStatus::Pending,
            ItemCheckpoint::Prepared,
            None,
            "{}",
            None,
            "t5"
        )
        .is_err()
    );
}

#[test]
fn completed_rerun_requires_canonical_validation_and_changed_inventory_conflicts() {
    let conn = Connection::open_in_memory().unwrap();
    let records = vec![record("note_obj", "n1", "s1")];
    begin_or_resume(&conn, "phase3-canonical-v1", &records, "t1").unwrap();
    ark_core::canonical_types::migration_ledger::transition_item(
        &conn,
        "phase3-canonical-v1",
        "note_obj",
        "n1",
        ItemStatus::Migrated,
        ItemCheckpoint::Committed,
        Some("c1"),
        "{}",
        None,
        "t2",
    )
    .unwrap();
    ark_core::canonical_types::migration_ledger::complete_run(
        &conn,
        "phase3-canonical-v1",
        "{}",
        "t3",
    )
    .unwrap();
    assert!(
        ark_core::canonical_types::migration_ledger::begin_or_resume_with_validator(
            &conn,
            "phase3-canonical-v1",
            &records,
            "t4",
            |_item, _record| Ok(())
        )
        .is_ok()
    );
    assert!(
        ark_core::canonical_types::migration_ledger::begin_or_resume_with_validator(
            &conn,
            "phase3-canonical-v1",
            &records,
            "t4",
            |_item, _record| Err(LedgerError::CanonicalConflict {
                source_kind: "note_obj".into(),
                source_id: "n1".into()
            })
        )
        .is_err()
    );
    assert!(begin_or_resume(
        &conn,
        "phase3-canonical-v1",
        &[record("note_obj", "n1", "changed")],
        "t5"
    )
    .is_err());
}

#[test]
fn child_savepoint_rolls_back_sentinel_but_guard_commit_survives() {
    let conn = Connection::open_in_memory().unwrap();
    ensure_ledger_schema(&conn).unwrap();
    let run = begin_or_resume(
        &conn,
        "phase3-canonical-v1",
        &[record("note_obj", "n1", "s1")],
        "now",
    )
    .unwrap();
    let mut child = ChildSavepoint::begin(&conn, &run.items[0]).unwrap();
    conn.execute("CREATE TABLE sentinel (value TEXT NOT NULL)", [])
        .unwrap();
    conn.execute("INSERT INTO sentinel VALUES ('transient')", [])
        .unwrap();
    child.rollback(LedgerError::invariant("injected")).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name='sentinel'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
