use super::migration::migrate_dirs;
use super::*;

use std::fs;

use tempfile::tempdir;

fn seed_legacy_dir(root: &std::path::Path) -> std::path::PathBuf {
    let legacy = root.join("Kosmos"); // MIGRATION(KOS-267)
    fs::create_dir_all(legacy.join("logs")).unwrap();
    fs::write(legacy.join("engine.lock.json"), b"{}").unwrap(); // MIGRATION(KOS-267)
    fs::write(legacy.join("kepler-device-id.txt"), b"dev-1").unwrap(); // MIGRATION(KOS-267)
    legacy
}

fn dirs(root: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    (root.join("Mundus"), root.join("Kosmos")) // MIGRATION(KOS-267)
}

#[test]
fn fresh_install_creates_nothing_and_does_not_migrate() {
    let roaming = tempdir().unwrap();
    let local = tempdir().unwrap();
    let (new, legacy) = dirs(roaming.path());
    let report = migrate_dirs(&new, &legacy, Some(local.path().join("Mundus")), None);
    assert!(!report.roaming_migrated);
    assert!(!report.fell_back_to_legacy);
    assert!(!new.exists());
}

#[test]
fn migrates_roaming_dir_and_writes_marker() {
    let roaming = tempdir().unwrap();
    let legacy = seed_legacy_dir(roaming.path());
    let (new, _) = dirs(roaming.path());
    let report = migrate_dirs(&new, &legacy, None, None);
    assert!(report.roaming_migrated);
    assert!(!report.fell_back_to_legacy);
    assert!(!legacy.exists());
    assert!(new.join("engine.lock.json").exists());
    assert!(new.join("kepler-device-id.txt").exists()); // MIGRATION(KOS-267)
    assert!(new.join("logs").is_dir());
    let marker = new.join(MIGRATION_MARKER);
    assert!(marker.exists());
    assert!(fs::read_to_string(marker).unwrap().contains("Mundus"));
}

#[test]
fn existing_new_dir_never_merges_legacy() {
    let roaming = tempdir().unwrap();
    let legacy = seed_legacy_dir(roaming.path());
    let (new, _) = dirs(roaming.path());
    fs::create_dir_all(&new).unwrap();
    fs::write(new.join("ours.txt"), b"x").unwrap();
    let report = migrate_dirs(&new, &legacy, None, None);
    assert!(!report.roaming_migrated);
    assert!(legacy.exists());
    assert!(!new.join("engine.lock.json").exists());
}

#[test]
fn locked_legacy_dir_falls_back_without_renaming() {
    let roaming = tempdir().unwrap();
    let legacy = seed_legacy_dir(roaming.path());
    let (new, _) = dirs(roaming.path());
    // A running 0.9.x Engine holds the legacy singleton lock.
    let guard = crate::singleton::SingletonGuard::acquire(&legacy.join(LEGACY_SINGLETON_LOCK_NAME))
        .unwrap();
    let report = migrate_dirs(&new, &legacy, None, None);
    assert!(report.fell_back_to_legacy);
    assert!(!report.roaming_migrated);
    assert!(legacy.exists());
    assert!(!new.exists());
    drop(guard);
}

#[test]
fn second_start_after_migration_is_a_noop() {
    let roaming = tempdir().unwrap();
    let legacy = seed_legacy_dir(roaming.path());
    let (new, _) = dirs(roaming.path());
    assert!(migrate_dirs(&new, &legacy, None, None).roaming_migrated);
    let report = migrate_dirs(&new, &legacy, None, None);
    assert!(!report.roaming_migrated);
    assert!(!report.fell_back_to_legacy);
    assert!(new.exists());
    assert!(!legacy.exists());
}

#[test]
fn moves_local_dir_entries_but_never_engine() {
    let local = tempdir().unwrap();
    let legacy_local = local.path().join("Kosmos"); // MIGRATION(KOS-267)
    fs::create_dir_all(legacy_local.join("Engine").join("versions")).unwrap();
    fs::write(legacy_local.join("Engine").join("current.json"), b"{}").unwrap();
    fs::create_dir_all(legacy_local.join("models")).unwrap();
    fs::write(legacy_local.join("cache.bin"), b"1").unwrap();
    let new_local = local.path().join("Mundus");
    // Installer already placed the new Engine payload.
    fs::create_dir_all(new_local.join("Engine")).unwrap();

    let roaming = tempdir().unwrap();
    let (new_roaming, legacy_roaming) = dirs(roaming.path());
    let report = migrate_dirs(
        &new_roaming,
        &legacy_roaming,
        Some(new_local.clone()),
        Some(legacy_local.clone()),
    );

    assert_eq!(report.local_entries_moved, 2);
    assert!(new_local.join("models").is_dir());
    assert!(new_local.join("cache.bin").exists());
    assert!(new_local.join("Engine").is_dir());
    // Legacy Engine payload stays; the installer removes it.
    assert!(legacy_local.join("Engine").is_dir());
    assert!(!legacy_local.join("models").exists());
}
