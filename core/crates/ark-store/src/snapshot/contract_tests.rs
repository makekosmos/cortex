// KOS-51: unit-level snapshot restore тесты. RPC-level покрытие —
// src/main/tests/snapshot_restore.rs.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::objects::list_objects;
use crate::snapshot::backup_to_file_chunked_verified as backup_to_file_chunked;
use rusqlite::Connection;
use std::fs;
use std::io::Write;

#[path = "legacy_startup_restore.rs"]
mod legacy_startup_restore;

fn setup_file_db() -> (crate::test_support::TestTempDir, String, Connection) {
    let dir = crate::test_support::test_temp_dir().unwrap();
    let db_path = dir.path().join("ark.db");
    let db_str = db_path.to_str().unwrap().to_string();
    let conn = crate::open::open_db(&db_str).unwrap();
    crate::open::init_schema(&conn).unwrap();
    (dir, db_str, conn)
}

fn make_object(id: &str, type_id: &str, title: &str) -> ark_domain::types::ArkObject {
    ark_domain::types::ArkObject {
        id: id.to_owned(),
        type_id: type_id.to_owned(),
        type_version: "0.0.0-legacy".to_owned(),
        title: title.to_owned(),
        content_json: serde_json::json!({"title": title}),
        props_json: serde_json::json!({}),
        created_at: "2026-01-01T00:00:00.000Z".to_owned(),
        updated_at: "2026-01-01T00:00:00.000Z".to_owned(),
        deleted_at: None,
    }
}

fn make_object_type(id: &str, name: &str) -> ark_domain::types::ObjectType {
    ark_domain::types::ObjectType {
        id: id.to_owned(),
        name: name.to_owned(),
        schema_json: "{}".to_owned(),
        ui_schema_json: "{}".to_owned(),
        created_at: "2026-01-01T00:00:00.000Z".to_owned(),
        updated_at: "2026-01-01T00:00:00.000Z".to_owned(),
        system_locked: false,
    }
}

fn seed_object(conn: &Connection, id: &str, title: &str) {
    crate::type_registry::upsert_object_type(conn, &make_object_type("snapshot_type", "Note"))
        .unwrap();
    crate::objects::upsert_object_compat(conn, &make_object(id, "snapshot_type", title)).unwrap();
}

/// Пишет snapshot текущей live DB в `<db_dir>/backups/<name>`.
fn make_snapshot(conn: &Connection, db_path: &str, name: &str) {
    let dir = backups_dir(db_path);
    fs::create_dir_all(&dir).unwrap();
    crate::snapshot::backup_connection_to_file(conn, dir.join(name).to_str().unwrap()).unwrap();
}

#[test]
fn snapshot_id_validation_accepts_only_plain_basenames() {
    assert!(validate_snapshot_id_for_test("ark.db.backup-2026-09-15").is_ok());
    for bad in [
        "",
        ".hidden",
        "..",
        "../ark.db",
        "..\\ark.db",
        "sub/dir.db",
        "sub\\dir.db",
        "C:\\abs.db",
        "/abs.db",
    ] {
        assert!(
            validate_snapshot_id_for_test(bad).is_err(),
            "rejected: {bad:?}"
        );
    }
}

#[test]
fn list_snapshots_returns_only_regular_files() {
    let (dir, db_path, conn) = setup_file_db();
    seed_object(&conn, "obj-1", "one");
    make_snapshot(&conn, &db_path, "ark.db.backup-a");
    // Noise: dot-staging file, nested dir, broken subpath — не попадают в list.
    let backups = backups_dir(&db_path);
    fs::write(backups.join(".restore-src-tmp.db"), b"junk").unwrap();
    fs::create_dir(backups.join("nested")).unwrap();

    let entries = list_snapshots(&db_path).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "ark.db.backup-a");
    assert!(entries[0].size_bytes > 0);
    drop(conn);
    drop(dir);
}

/// AC5: провал post-restore verification откатывает live DB в
/// pre-restore состояние — НЕ в состояние snapshot'а и не в кашу.
#[test]
fn restore_rolls_back_to_pre_restore_state_when_post_verify_fails() {
    let (_dir, db_path, mut conn) = setup_file_db();
    seed_object(&conn, "obj-snap", "in snapshot");
    make_snapshot(&conn, &db_path, "ark.db.backup-rollback");

    // Live DB расходится со snapshot'ом: объект удалён ПОСЛЕ backup'а.
    conn.execute("DELETE FROM objects WHERE id = 'obj-snap'", [])
        .unwrap();
    assert!(list_objects(&conn).unwrap().is_empty());

    let failing_verify = |_conn: &Connection, _fp: &[String]| -> Result<(), String> {
        Err("injected post-verify failure".to_string())
    };
    let result = restore_snapshot_with_hooks(
        &mut conn,
        &db_path,
        "ark.db.backup-rollback",
        failing_verify,
        default_rollback_restore_for_test,
    );
    let err = result.expect_err("post-verify failure must fail the restore");
    assert!(
        err.contains("rolled back"),
        "error must report rollback: {err}"
    );

    // Rollback = pre-restore состояние (объект удалён), не snapshot.
    assert!(
        list_objects(&conn).unwrap().is_empty(),
        "rollback must restore the PRE-RESTORE live state, not the snapshot"
    );
    crate::open::check_integrity(&conn).unwrap();

    // Успешный откат подчищает staging и rollback — в backups/ остаётся
    // только исходный snapshot.
    let leftovers: Vec<String> = fs::read_dir(backups_dir(&db_path))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(leftovers, vec!["ark.db.backup-rollback".to_string()]);
}

/// KOS-83: провал post-verify + провал rollback — pre-restore snapshot
/// НЕ удаляется, а сохраняется под видимым basename'ом и остаётся
/// валидной точкой восстановления pre-restore данных.
#[test]
fn restore_preserves_rollback_snapshot_when_verify_and_rollback_both_fail() {
    let (_dir, db_path, mut conn) = setup_file_db();
    seed_object(&conn, "obj-snap", "in snapshot");
    make_snapshot(&conn, &db_path, "ark.db.backup-dblfail");

    // Live DB расходится со snapshot'ом: объект удалён ПОСЛЕ backup'а.
    conn.execute("DELETE FROM objects WHERE id = 'obj-snap'", [])
        .unwrap();

    let failing_verify = |_conn: &Connection, _fp: &[String]| -> Result<(), String> {
        Err("injected post-verify failure".to_string())
    };
    let failing_rollback = |_conn: &mut Connection, _path: &Path| -> Result<(), String> {
        Err("injected rollback failure".to_string())
    };
    let result = restore_snapshot_with_hooks(
        &mut conn,
        &db_path,
        "ark.db.backup-dblfail",
        failing_verify,
        failing_rollback,
    );
    let err = result.expect_err("double failure must fail the restore");
    assert!(
        err.contains("rollback failed") && err.contains("pre-restore snapshot preserved"),
        "error must surface preserved snapshot: {err}"
    );

    // Pre-restore snapshot сохранён под видимым для list/restore именем;
    // hidden rollback-файла и staging-копии не осталось.
    let names: Vec<String> = fs::read_dir(backups_dir(&db_path))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    let preserved: Vec<&String> = names
        .iter()
        .filter(|name| name.starts_with("ark.db.pre-restore-failed-"))
        .collect();
    assert_eq!(
        preserved.len(),
        1,
        "expected one preserved snapshot: {names:?}"
    );
    assert!(
        !names.iter().any(|name| name.starts_with('.')),
        "hidden staging/rollback files must not be left behind: {names:?}"
    );
    assert!(
        err.contains(preserved[0].as_str()),
        "error must name the preserved snapshot: {err}"
    );
    let listed = list_snapshots(&db_path).unwrap();
    assert!(
        listed.iter().any(|entry| entry.id == *preserved[0]),
        "preserved snapshot must be visible via list_snapshots"
    );

    // Содержимое — pre-restore состояние live DB (объект удалён),
    // а не данные snapshot'а.
    let preserved_path = backups_dir(&db_path).join(preserved[0]);
    let preserved_conn = Connection::open(&preserved_path).unwrap();
    crate::open::check_integrity(&preserved_conn).unwrap();
    assert!(
        list_objects(&preserved_conn).unwrap().is_empty(),
        "preserved snapshot must hold the PRE-RESTORE state"
    );
    drop(preserved_conn);

    drop(conn);
    crate::snapshot::recover_interrupted_restore(&db_path).unwrap();
    let mut conn = crate::open::open_db(&db_path).unwrap();
    let report = restore_snapshot(&mut conn, &db_path, preserved[0]).unwrap();
    assert!(report.restored);
    assert_eq!(report.objects, 0);
    assert!(
        list_objects(&conn).unwrap().is_empty(),
        "restoring the preserved snapshot must return live DB to pre-restore state"
    );
    crate::open::check_integrity(&conn).unwrap();
}

#[test]
fn restore_rejects_traversal_and_missing_ids_without_touching_live() {
    let (_dir, db_path, mut conn) = setup_file_db();
    seed_object(&conn, "obj-live", "alive");
    make_snapshot(&conn, &db_path, "ark.db.backup-ok");

    for bad in ["../ark.db", "sub/dir.db", "missing.db"] {
        let result = restore_snapshot(&mut conn, &db_path, bad);
        assert!(result.is_err(), "must reject {bad:?}");
    }
    // Traversal на живой файл тоже не должен его трогать.
    let objects = list_objects(&conn).unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].id, "obj-live");
}

#[test]
fn validate_reports_integrity_and_schema_verdicts() {
    let (_dir, db_path, conn) = setup_file_db();
    seed_object(&conn, "obj-v", "v");
    make_snapshot(&conn, &db_path, "ark.db.backup-valid");

    let ok = validate_snapshot(&conn, &db_path, "ark.db.backup-valid").unwrap();
    assert!(ok.exists && ok.integrity_ok && ok.schema_match && ok.valid);

    // Не-SQLite garbage.
    fs::write(
        backups_dir(&db_path).join("ark.db.backup-garbage"),
        b"not a sqlite db",
    )
    .unwrap();
    let garbage = validate_snapshot(&conn, &db_path, "ark.db.backup-garbage").unwrap();
    assert!(garbage.exists);
    assert!(!garbage.integrity_ok);
    assert!(!garbage.valid);

    // Валидный SQLite, но чужая schema.
    let foreign_path = backups_dir(&db_path).join("ark.db.backup-foreign");
    let foreign = Connection::open(&foreign_path).unwrap();
    foreign
        .execute_batch("CREATE TABLE other(id TEXT PRIMARY KEY)")
        .unwrap();
    drop(foreign);
    let mismatched = validate_snapshot(&conn, &db_path, "ark.db.backup-foreign").unwrap();
    assert!(mismatched.integrity_ok);
    assert!(!mismatched.schema_match);
    assert!(!mismatched.valid);

    let missing = validate_snapshot(&conn, &db_path, "ark.db.backup-nope").unwrap();
    assert!(!missing.exists && !missing.valid);
}

#[test]
fn verified_backup_is_published_with_manifest_and_tampering_is_rejected() {
    let (_dir, db_path, conn) = setup_file_db();
    seed_object(&conn, "obj-manifest", "manifest checked");
    fs::create_dir_all(backups_dir(&db_path)).unwrap();
    let id = "ark.db.backup-v1-2026-10-17-120000";
    let destination = backups_dir(&db_path).join(id);
    backup_to_file_chunked(
        &db_path,
        destination.to_str().unwrap(),
        4,
        std::time::Duration::ZERO,
    )
    .unwrap();
    assert!(destination.exists());
    assert!(destination
        .with_file_name(format!("{id}.manifest.json"))
        .exists());

    let listed = list_snapshots(&db_path).unwrap();
    assert!(listed.iter().any(|entry| entry.id == id));
    assert!(validate_snapshot(&conn, &db_path, id).unwrap().valid);

    fs::OpenOptions::new()
        .append(true)
        .open(&destination)
        .unwrap()
        .write_all(b"tampered")
        .unwrap();
    assert!(validate_snapshot(&conn, &db_path, id)
        .unwrap_err()
        .contains("checksum mismatch"));
}

#[test]
fn verified_backup_orphan_without_manifest_is_hidden_and_not_restorable() {
    let (_dir, db_path, conn) = setup_file_db();
    seed_object(&conn, "obj-orphan", "orphan");
    fs::create_dir_all(backups_dir(&db_path)).unwrap();
    let id = "ark.db.backup-v1-2026-10-17-120001";
    let destination = backups_dir(&db_path).join(id);
    backup_to_file_chunked(
        &db_path,
        destination.to_str().unwrap(),
        4,
        std::time::Duration::ZERO,
    )
    .unwrap();
    fs::remove_file(destination.with_file_name(format!("{id}.manifest.json"))).unwrap();

    assert!(!list_snapshots(&db_path)
        .unwrap()
        .iter()
        .any(|entry| entry.id == id));
    assert!(validate_snapshot(&conn, &db_path, id)
        .unwrap_err()
        .contains("manifest unavailable"));
}

#[test]
fn verified_backup_publication_is_no_clobber_and_ignores_manifest_only_orphans() {
    let (_dir, db_path, conn) = setup_file_db();
    fs::create_dir_all(backups_dir(&db_path)).unwrap();
    let backups = backups_dir(&db_path);
    let db_id = "ark.db.backup-v1-collision-db";
    let db_destination = backups.join(db_id);
    fs::write(&db_destination, b"existing database owner").unwrap();
    let error = backup_to_file_chunked(
        &db_path,
        db_destination.to_str().unwrap(),
        4,
        std::time::Duration::ZERO,
    )
    .expect_err("existing database destination must not be replaced");
    assert!(error.contains("already exists"));
    assert_eq!(
        fs::read(&db_destination).unwrap(),
        b"existing database owner"
    );

    let manifest_id = "ark.db.backup-v1-collision-manifest";
    let manifest_path = backups.join(format!("{manifest_id}.manifest.json"));
    fs::write(&manifest_path, b"existing manifest owner").unwrap();
    let manifest_destination = backups.join(manifest_id);
    let error = backup_to_file_chunked(
        &db_path,
        manifest_destination.to_str().unwrap(),
        4,
        std::time::Duration::ZERO,
    )
    .expect_err("existing manifest destination must not be replaced");
    assert!(error.contains("already exists"));
    assert_eq!(
        fs::read(&manifest_path).unwrap(),
        b"existing manifest owner"
    );
    assert!(!manifest_destination.exists());

    // Crash after manifest publication but before database publication leaves
    // an orphan marker. It must remain invisible and unrestorable.
    let orphan_id = "ark.db.backup-v1-manifest-orphan";
    let orphan_manifest = backups.join(format!("{orphan_id}.manifest.json"));
    fs::write(&orphan_manifest, b"partial manifest publication").unwrap();
    assert!(!list_snapshots(&db_path)
        .unwrap()
        .iter()
        .any(|entry| entry.id == orphan_id));
    let orphan_validation = validate_snapshot(&conn, &db_path, orphan_id).unwrap();
    assert!(!orphan_validation.exists && !orphan_validation.valid);
    assert!(orphan_validation
        .error
        .as_deref()
        .is_some_and(|error| error.contains("snapshot not found")));
}

#[test]
fn restore_replaces_live_content_and_stays_valid() {
    let (_dir, db_path, mut conn) = setup_file_db();
    seed_object(&conn, "obj-a", "before");
    make_snapshot(&conn, &db_path, "ark.db.backup-1");

    // Расходимся: удаляем snapshot-объект, добавляем новый.
    conn.execute("DELETE FROM objects WHERE id = 'obj-a'", [])
        .unwrap();
    crate::objects::upsert_object_compat(&conn, &make_object("obj-b", "snapshot_type", "after"))
        .unwrap();

    let report = restore_snapshot(&mut conn, &db_path, "ark.db.backup-1").unwrap();
    assert!(report.restored);
    assert_eq!(report.objects, 1);

    let objects = list_objects(&conn).unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].id, "obj-a");
    crate::open::check_integrity(&conn).unwrap();
}
