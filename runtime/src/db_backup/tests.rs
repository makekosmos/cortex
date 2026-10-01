use super::*;

#[test]
fn parses_backup_timestamp_round_trip() {
    let ts = DateTime::parse_from_rfc3339("2026-05-18T15:30:45Z")
        .unwrap()
        .with_timezone(&Utc);
    let name = backup_filename(ts);
    let parsed = parse_backup_timestamp(&name).expect("должен распарситься");
    assert_eq!(parsed, ts);
}

#[test]
fn rejects_non_backup_filename() {
    assert!(parse_backup_timestamp("ark.db").is_none());
    assert!(parse_backup_timestamp("ark.db.backup-not-a-date").is_none());
    assert!(parse_backup_timestamp("random.txt").is_none());
}

#[test]
fn snapshot_id_accepts_only_backup_basenames() {
    let params = serde_json::json!({ "backup_id": "ark.db.backup-2026-09-16-153045" });
    assert_eq!(
        snapshot_id(&params).unwrap(),
        "ark.db.backup-2026-09-16-153045"
    );
    for bad in [
        "",
        "ark.db",
        "ark.db.backup-2026-09-16",
        "ark.db.backup-not-a-date",
        ".restore-rollback-1.db",
        "../ark.db.backup-2026-09-16-153045",
        "sub/ark.db.backup-2026-09-16-153045",
        "ark.db.backup-2026-09-16-153045.db",
    ] {
        let params = serde_json::json!({ "backup_id": bad });
        assert!(snapshot_id(&params).is_err(), "{bad} must be rejected");
    }
    assert!(snapshot_id(&serde_json::json!({})).is_err());
    assert!(snapshot_id(&serde_json::json!({ "backup_id": 42 })).is_err());
}

#[test]
fn rejects_linked_backups_directory() {
    let root = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let backups = root.path().join(BACKUPS_SUBDIR);
    crate::test_links::link_dir(target.path(), &backups).expect("junction");

    assert!(ensure_backups_dir(root.path()).is_err());
}

#[test]
fn allocates_a_unique_name_when_second_resolution_collides() {
    let dir = tempfile::tempdir().unwrap();
    let now = Utc::now();
    std::fs::write(dir.path().join(backup_filename(now)), b"existing").unwrap();

    let (allocated, path) = next_backup_destination(dir.path(), now).unwrap();
    assert_eq!(allocated, now + chrono::Duration::seconds(1));
    assert_eq!(path, dir.path().join(backup_filename(allocated)));
}

#[test]
fn diagnostics_exposes_db_backup_background_worker() {
    // Regression: 2026-06-09. diagnostics.snapshot must expose DB backup
    // background worker state and chunking config in background_workers.
    let snapshot = diagnostics_snapshot();

    assert!(!snapshot.active);
    assert!(snapshot.pages_per_step > 0);
    assert!(snapshot.background_mode);
}
