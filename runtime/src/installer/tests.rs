//! Ports of the deleted PowerShell test suites
//! (engine-install-headless.test.mjs / engine-post-install-headless.test.mjs)
//! against an isolated temp target root — every behaviour the scripts had a
//! test for keeps one here (KOS-306). Registry-touching tests point at
//! scratch HKCU subkeys, never the real Run/Uninstall state.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::manifest::sha256_hex;
use super::*;

fn write_manifest(payload: &Path, version: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let entries: Vec<serde_json::Value> = files
        .iter()
        .map(|(name, data)| {
            let path = payload.join(name);
            serde_json::json!({
                "name": name,
                "sha256": sha256_hex(&path).unwrap(),
                "size": data.len(),
            })
        })
        .collect();
    let manifest = serde_json::json!({
        "schema_version": 1,
        "product": "mundus-engine",
        "version": version,
        "source_commit": "a".repeat(40),
        "files": entries,
    });
    let path = payload.join("engine-manifest.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    path
}

/// A staged payload dir (exe + tray.ico + manifest) plus an empty target
/// root — what the NSIS stage hands to `install`.
fn fixture(version: &str) -> (TempDir, PathBuf, PathBuf, install::Options) {
    let root = TempDir::new().unwrap();
    let payload = root.path().join("payload");
    std::fs::create_dir_all(&payload).unwrap();
    for (name, data) in [
        ("mundus-engine.exe", b"fixture:mundus-engine.exe".as_slice()),
        ("tray.ico", b"fixture:tray.ico".as_slice()),
    ] {
        std::fs::write(payload.join(name), data).unwrap();
    }
    let manifest = write_manifest(
        &payload,
        version,
        &[
            ("mundus-engine.exe", b"fixture:mundus-engine.exe"),
            ("tray.ico", b"fixture:tray.ico"),
        ],
    );
    let target_root = root.path().join("installed");
    let options = install::Options {
        manifest,
        target_root: target_root.clone(),
        // A scratch key that does not exist: the migration short-circuits.
        legacy_registry_key: format!(
            r"Software\MundusInstallerTest\{}\absent",
            std::process::id()
        ),
        legacy_shortcut: None,
    };
    (root, payload, target_root, options)
}

fn current_version(target_root: &Path) -> String {
    let pointer: serde_json::Value =
        serde_json::from_slice(&std::fs::read(target_root.join("current.json")).unwrap()).unwrap();
    pointer["version"].as_str().unwrap().to_owned()
}

#[test]
fn fresh_install_copies_verifies_and_points_current_at_the_bundled_version() {
    let (_root, _payload, target_root, options) = fixture("1.2.3");
    let report = install::install(&options).unwrap();
    assert_eq!(report["action"], "installed");
    assert_eq!(current_version(&target_root), "1.2.3");
    let backend = target_root.join("versions/1.2.3/mundus-engine.exe");
    assert_eq!(
        std::fs::read(&backend).unwrap(),
        b"fixture:mundus-engine.exe"
    );
    assert!(target_root
        .join("versions/1.2.3/engine-manifest.json")
        .is_file());
}

#[test]
fn install_is_idempotent_and_repairs_a_corrupted_installation() {
    let (_root, _payload, target_root, options) = fixture("1.2.3");
    assert_eq!(install::install(&options).unwrap()["action"], "installed");
    // Same build again: kept, not recopied.
    assert_eq!(install::install(&options).unwrap()["action"], "kept");
    let backend = target_root.join("versions/1.2.3/mundus-engine.exe");
    std::fs::write(&backend, b"corrupt").unwrap();
    assert_eq!(install::install(&options).unwrap()["action"], "installed");
    assert_eq!(
        std::fs::read(&backend).unwrap(),
        b"fixture:mundus-engine.exe"
    );
}

#[test]
fn install_never_downgrades_a_newer_verified_engine() {
    let (_root, _payload, target_root, options) = fixture("2.0.0");
    install::install(&options).unwrap();
    let older = TempDir::new().unwrap();
    let payload = older.path().join("payload");
    std::fs::create_dir_all(&payload).unwrap();
    for (name, data) in [
        ("mundus-engine.exe", b"older:mundus-engine.exe".as_slice()),
        ("tray.ico", b"older:tray.ico".as_slice()),
    ] {
        std::fs::write(payload.join(name), data).unwrap();
    }
    let manifest = write_manifest(
        &payload,
        "1.0.0",
        &[
            ("mundus-engine.exe", b"older:mundus-engine.exe"),
            ("tray.ico", b"older:tray.ico"),
        ],
    );
    let older_options = install::Options {
        manifest,
        target_root: target_root.clone(),
        ..options
    };
    assert_eq!(install::install(&older_options).unwrap()["action"], "kept");
    assert_eq!(current_version(&target_root), "2.0.0");
    assert!(!target_root.join("versions/1.0.0").exists());
}

#[test]
fn a_same_version_rebuild_replaces_the_installed_engine() {
    let (_root, _payload, target_root, options) = fixture("1.2.3");
    install::install(&options).unwrap();
    let backend = target_root.join("versions/1.2.3/mundus-engine.exe");
    assert_eq!(
        std::fs::read(&backend).unwrap(),
        b"fixture:mundus-engine.exe"
    );

    let (_rebuild, rebuild_options) = same_version_rebuild(options);
    assert_eq!(
        install::install(&rebuild_options).unwrap()["action"],
        "installed"
    );
    assert_eq!(
        std::fs::read(&backend).unwrap(),
        b"rebuild:mundus-engine.exe"
    );
    assert_eq!(current_version(&target_root), "1.2.3");
}

/// A same-version rebuild payload ("rebuild:*" bytes) for `options` —
/// exercises the `versions/<v>` swap path. The TempDir must outlive the
/// call to `install`.
fn same_version_rebuild(options: install::Options) -> (TempDir, install::Options) {
    let rebuild = TempDir::new().unwrap();
    let payload = rebuild.path().join("payload");
    std::fs::create_dir_all(&payload).unwrap();
    for (name, data) in [
        ("mundus-engine.exe", b"rebuild:mundus-engine.exe".as_slice()),
        ("tray.ico", b"rebuild:tray.ico".as_slice()),
    ] {
        std::fs::write(payload.join(name), data).unwrap();
    }
    let manifest = write_manifest(
        &payload,
        "1.2.3",
        &[
            ("mundus-engine.exe", b"rebuild:mundus-engine.exe"),
            ("tray.ico", b"rebuild:tray.ico"),
        ],
    );
    (
        rebuild,
        install::Options {
            manifest,
            ..options
        },
    )
}

#[test]
fn a_failed_move_in_restores_the_previous_install() {
    // KOS-306 round 2: the second rename of a same-version replace is forced
    // to fail through the move_in seam — the previously installed Engine
    // must still be intact afterwards, not deleted.
    let (_root, _payload, target_root, options) = fixture("1.2.3");
    install::install(&options).unwrap();
    let backend = target_root.join("versions/1.2.3/mundus-engine.exe");
    assert_eq!(
        std::fs::read(&backend).unwrap(),
        b"fixture:mundus-engine.exe"
    );

    let (_rebuild, rebuild_options) = same_version_rebuild(options);
    let _fail_move = swap::FailNextMoveIn::arm();
    assert!(install::install(&rebuild_options).is_err());
    assert_eq!(
        std::fs::read(&backend).unwrap(),
        b"fixture:mundus-engine.exe",
        "previous install must survive a failed replace"
    );
    assert_eq!(current_version(&target_root), "1.2.3");
    assert!(
        !target_root
            .join("versions")
            .join(format!("1.2.3.{}.old", std::process::id()))
            .exists(),
        "the aside dir is renamed back, not left behind"
    );
}

#[test]
fn a_failed_restore_reports_where_the_engine_stayed() {
    // KOS-306 round 3: when even the restore rename fails, the error must
    // name the aside dir — otherwise the previous Engine is silently lost.
    let (_root, _payload, target_root, options) = fixture("1.2.3");
    install::install(&options).unwrap();

    let (_rebuild, rebuild_options) = same_version_rebuild(options);
    let _fail_move = swap::FailNextMoveIn::arm();
    let _fail_restore = swap::FailNextRestore::arm();
    let error = install::install(&rebuild_options).unwrap_err();
    let aside = target_root
        .join("versions")
        .join(format!("1.2.3.{}.old", std::process::id()));
    assert!(error.contains("previous Engine kept at"), "{error}");
    assert!(
        aside.join("mundus-engine.exe").is_file(),
        "the previous Engine still exists at the reported aside path"
    );
}

#[test]
fn tampered_payload_blocks_installation() {
    let (_root, payload, target_root, options) = fixture("1.2.3");
    std::fs::write(payload.join("mundus-engine.exe"), b"tampered").unwrap();
    assert!(install::install(&options).is_err());
    assert!(!target_root.join("current.json").exists());
}

#[test]
fn unsafe_manifest_paths_and_sizes_are_rejected_before_installation() {
    for (name, size) in [("../escape.exe", 1u64), ("..", 1), ("ok.exe", 0)] {
        let root = TempDir::new().unwrap();
        let payload = root.path().join("payload");
        std::fs::create_dir_all(&payload).unwrap();
        let manifest_path = payload.join("engine-manifest.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "product": "mundus-engine",
                "version": "1.2.3",
                "files": [{ "name": name, "sha256": "0".repeat(64), "size": size }],
            }))
            .unwrap(),
        )
        .unwrap();
        let options = install::Options {
            manifest: manifest_path,
            target_root: root.path().join("installed"),
            legacy_registry_key: format!(
                r"Software\MundusInstallerTest\{}\absent",
                std::process::id()
            ),
            legacy_shortcut: None,
        };
        // "ok.exe" fails too — the declared payload file does not exist.
        assert!(install::install(&options).is_err(), "{name}");
        assert!(!root.path().join("escape.exe").exists());
    }
}

#[cfg(windows)]
#[test]
fn install_takes_over_an_existing_standalone_kosmos_engine_registration() {
    let (_root, _payload, target_root, mut options) = fixture("1.2.3");
    let scratch = format!(
        r"Software\MundusInstallerTest\{}\takeover",
        std::process::id()
    );
    let shortcut = _root.path().join("Kosmos Engine.lnk"); // MIGRATION(KOS-267)
    std::fs::write(&shortcut, b"fake shortcut").unwrap();
    let old_root = _root.path().join("old-standalone-engine");
    std::fs::create_dir_all(&old_root).unwrap();
    std::fs::write(old_root.join("Uninstall.exe"), b"old uninstaller").unwrap();
    registry::create_key(&scratch).unwrap();
    registry::write_sz(&scratch, "DisplayName", "Kosmos Engine").unwrap();
    registry::write_sz(&scratch, "InstallLocation", &old_root.to_string_lossy()).unwrap();
    options.legacy_registry_key = scratch.clone();
    options.legacy_shortcut = Some(shortcut.clone());

    install::install(&options).unwrap();
    assert!(!registry::key_exists(&scratch).unwrap());
    assert!(!shortcut.exists(), "legacy Start Menu shortcut removed");
    assert!(
        !old_root.join("Uninstall.exe").exists(),
        "orphaned standalone uninstaller cleaned up"
    );
    assert_eq!(current_version(&target_root), "1.2.3");
    let _ = registry::delete_tree(&scratch);
}

fn post_install_options(root: &Path) -> post_install::Options {
    let pid = std::process::id();
    post_install::Options {
        engine_root: root.to_path_buf(),
        run_key: format!(r"Software\MundusInstallerTest\{pid}\Run"),
        startup_approved_key: format!(r"Software\MundusInstallerTest\{pid}\Approved"),
        migrate_autostart: false,
        start_engine: false,
    }
}

fn installed_root(version: &str) -> TempDir {
    let root = TempDir::new().unwrap();
    let version_dir = root.path().join("versions").join(version);
    std::fs::create_dir_all(&version_dir).unwrap();
    std::fs::write(version_dir.join("mundus-engine.exe"), b"fixture").unwrap();
    std::fs::write(
        root.path().join("current.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "version": version,
        }))
        .unwrap(),
    )
    .unwrap();
    root
}

#[test]
fn valid_current_json_resolves_the_installed_engine_exe() {
    let root = installed_root("2.4.6");
    let exe = post_install::resolve_engine_exe(root.path()).unwrap();
    assert!(exe.ends_with(r"versions\2.4.6\mundus-engine.exe"));
}

#[test]
fn missing_or_invalid_current_json_fails_closed() {
    for pointer in [
        serde_json::json!({ "schema_version": 2, "version": "1.2.3" }),
        serde_json::json!({ "schema_version": 1, "version": "not-a-version" }),
        serde_json::json!({ "schema_version": 1, "version": "../escape" }),
    ] {
        let root = TempDir::new().unwrap();
        std::fs::write(
            root.path().join("current.json"),
            serde_json::to_vec(&pointer).unwrap(),
        )
        .unwrap();
        assert!(post_install::resolve_engine_exe(root.path()).is_err());
    }
    let empty = TempDir::new().unwrap();
    assert!(post_install::resolve_engine_exe(empty.path()).is_err());
}

#[test]
fn pointed_at_version_without_an_exe_fails_closed() {
    let root = TempDir::new().unwrap();
    std::fs::write(
        root.path().join("current.json"),
        br#"{"schema_version":1,"version":"9.9.9"}"#,
    )
    .unwrap();
    assert!(post_install::resolve_engine_exe(root.path()).is_err());
}

#[cfg(windows)]
#[test]
fn migrate_autostart_seeds_run_value_and_enabled_marker() {
    let root = installed_root("3.0.0");
    let mut options = post_install_options(root.path());
    options.migrate_autostart = true;
    post_install::post_install(&options).unwrap();
    let exe = post_install::resolve_engine_exe(root.path()).unwrap();
    assert_eq!(
        registry::read_sz(&options.run_key, "Mundus Engine").as_deref(),
        Some(format!("\"{}\" --start", exe.display()).as_str())
    );
    let marker = registry::read(&options.startup_approved_key, "Mundus Engine")
        .unwrap()
        .expect("enabled marker written");
    assert_eq!(marker.data.first(), Some(&2));
    registry::delete_value(&options.run_key, "Mundus Engine");
    registry::delete_value(&options.startup_approved_key, "Mundus Engine");
}

#[cfg(windows)]
#[test]
fn disabled_startup_approved_marker_under_a_legacy_name_skips_autostart() {
    for (marker_name, state) in [("Mundus Engine", 3u8), ("Kosmos Engine", 6u8)] {
        let root = installed_root("1.2.3");
        let mut options = post_install_options(root.path());
        options.migrate_autostart = true;
        options.startup_approved_key = format!(
            r"Software\MundusInstallerTest\{}\Approved-{marker_name}",
            std::process::id()
        );
        options.run_key = format!(
            r"Software\MundusInstallerTest\{}\Run-{marker_name}",
            std::process::id()
        );
        registry::write_binary(
            &options.startup_approved_key,
            marker_name,
            &[state, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        )
        .unwrap();
        let report = post_install::post_install(&options).unwrap();
        assert_eq!(
            report["autostart"].as_str().unwrap(),
            format!("skipped opt-out={marker_name}")
        );
        assert!(
            registry::read_sz(&options.run_key, "Mundus Engine").is_none(),
            "opt-out must not write a Run value"
        );
        registry::delete_value(&options.startup_approved_key, marker_name);
    }
}

#[test]
fn start_engine_surfaces_a_spawn_failure_instead_of_silently_passing() {
    let root = installed_root("1.2.3");
    let mut options = post_install_options(root.path());
    options.start_engine = true;
    // The fixture exe is a plain text file: the spawn fails and the error
    // must reach the caller as a failed outcome.
    assert!(post_install::post_install(&options).is_err());
}

#[test]
fn non_installer_argv_is_not_intercepted() {
    assert!(run_if_installer(&[]).is_none());
    assert!(run_if_installer(&["--start".to_string()]).is_none());
    assert!(run_if_installer(&["--core-worker".to_string()]).is_none());
}
