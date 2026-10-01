use super::*;

fn put(dir: &Path, rel: &str, bytes: usize) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, vec![0_u8; bytes]).unwrap();
}

fn bytes_of(value: &Value, id: &str) -> u64 {
    value["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .map(|c| c["bytes"].as_u64().unwrap())
        .unwrap_or(0)
}

/// The layout mirrors the real data dir: ark.db sidecars, backups,
/// dictation models, package store, updates, indexes and stray files.
#[test]
fn storage_breakdown_splits_known_dirs_and_buckets_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    // A real sqlite db so dbstat can attribute pages.
    let conn = rusqlite::Connection::open(root.join("ark.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE objects (id TEXT PRIMARY KEY, payload TEXT);
         CREATE TABLE usage_sync_log (id INTEGER PRIMARY KEY, blob TEXT);
         INSERT INTO objects VALUES ('o1', printf('%.*c', 900, 'x'));
         INSERT INTO usage_sync_log (blob) VALUES (printf('%.*c', 4000, 'y'));",
    )
    .unwrap();
    drop(conn);
    put(root, "ark.db-wal", 10);
    put(root, "backups/ark-backup.db", 100);
    put(root, "backups/ark.db-shm", 5);
    put(root, "models/dictation/ggml.bin", 2200);
    put(root, "models/other.bin", 30);
    put(root, "packages/blobs/deadbeef.kspkg", 400);
    put(root, "updates/Mundus-Setup-0.9.37.exe", 317);
    put(root, "file-index.db", 120);
    put(root, "app-index.db-wal", 18);
    put(root, "legacy-quarantine/old.zip", 124);
    put(root, "stray.tmp.1", 7);

    let breakdown = storage_breakdown(root, &root.join("packages"));
    let categories = breakdown["categories"].as_array().unwrap();
    let db_bytes = bytes_of(&breakdown, "database");
    assert!(db_bytes > 10, "ark.db pages + wal");
    assert_eq!(bytes_of(&breakdown, "backups"), 105);
    assert_eq!(bytes_of(&breakdown, "dictation_models"), 2200);
    assert_eq!(bytes_of(&breakdown, "packages"), 400);
    assert_eq!(bytes_of(&breakdown, "updates"), 317);
    assert_eq!(bytes_of(&breakdown, "file_index"), 120);
    assert_eq!(bytes_of(&breakdown, "app_index"), 18);
    assert_eq!(bytes_of(&breakdown, "other"), 124 + 7 + 30);
    assert_eq!(
        breakdown["total_bytes"].as_u64().unwrap(),
        categories
            .iter()
            .map(|c| c["bytes"].as_u64().unwrap())
            .sum::<u64>(),
        "total is the sum of categories, no hidden bytes"
    );

    let detail = categories.iter().find(|c| c["id"] == "database").unwrap()["detail"]
        .as_array()
        .unwrap()
        .clone();
    let usage = detail.iter().find(|d| d["id"] == "usage_tracker").unwrap()["bytes"]
        .as_u64()
        .unwrap();
    let objects = detail.iter().find(|d| d["id"] == "objects").unwrap()["bytes"]
        .as_u64()
        .unwrap();
    assert!(usage > 0, "usage_sync_log pages attributed");
    assert!(objects > 0, "objects pages attributed");
}

#[test]
fn storage_breakdown_on_empty_dir_reports_zero() {
    let dir = tempfile::tempdir().unwrap();
    let breakdown = storage_breakdown(dir.path(), &dir.path().join("packages"));
    assert_eq!(breakdown["total_bytes"], 0);
    // Database is always listed so the UI has a stable anchor.
    assert!(breakdown["categories"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == "database"));
}

/// A corrupt or non-sqlite ark.db must not kill the breakdown: categories
/// still carry their byte counts and the database entry just has no detail.
#[test]
fn storage_breakdown_survives_a_non_sqlite_ark_db() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    put(root, "ark.db", 4096); // zeroed pages — not a sqlite header
    put(root, "updates/x.bin", 5);
    let breakdown = storage_breakdown(root, &root.join("packages"));
    let database = breakdown["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "database")
        .unwrap();
    assert_eq!(database["bytes"].as_u64().unwrap(), 4096);
    assert!(
        database.get("detail").is_none(),
        "corrupt db must omit detail, got {database}"
    );
    assert_eq!(bytes_of(&breakdown, "updates"), 5);
}

/// A directory symlink pointing back at its parent must be counted once as
/// a link and never descended into — no infinite recursion, no double count.
#[test]
fn storage_breakdown_does_not_descend_into_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    put(root, "real/payload.bin", 64);
    crate::test_links::link_dir(&root.join("real"), &root.join("link")).expect("junction");
    let breakdown = storage_breakdown(root, &root.join("packages"));
    let other = bytes_of(&breakdown, "other");
    assert_eq!(
        other, 64,
        "link counted once, target payload not doubled: {breakdown}"
    );
    // The walk completing at all is the point — a descent into `link` would
    // have made it 128+ or hung.
}

#[tokio::test]
async fn data_storage_runs_off_the_request_loop() {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "updates/x.bin", 5);
    let state = ManagerState::new(dir.path().to_path_buf());
    let value = state
        .data_storage(&dir.path().join("packages"))
        .await
        .unwrap();
    assert_eq!(bytes_of(&value, "updates"), 5);
}
