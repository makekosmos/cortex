use super::*;

#[test]
fn bundle_for_exe_accepts_the_dmg_layout_and_rejects_bare_binaries() {
    let app = bundle_for_exe(Path::new(
        "/Applications/Mundus Manager.app/Contents/MacOS/mundus-engine",
    ))
    .unwrap();
    assert_eq!(app, Path::new("/Applications/Mundus Manager.app"));

    for exe in [
        "/usr/local/bin/mundus-engine",
        "/Applications/mundus-engine",
        "/tmp/Mundus Manager.app/mundus-engine",
        "mundus-engine",
    ] {
        assert!(bundle_for_exe(Path::new(exe)).is_err(), "{exe}");
    }
}

#[test]
fn plan_stages_everything_next_to_the_app_for_a_same_volume_rename() {
    let app = Path::new("/Applications/Mundus Manager.app");
    let plan = MacInstallPlan::for_app(app).unwrap();
    assert_eq!(plan.app, app);
    assert_eq!(plan.staging_dir.parent(), app.parent());
    assert_eq!(plan.staged_app.extension().unwrap(), "app");
    assert_eq!(plan.staged_app.file_name(), app.file_name());
    for path in [
        &plan.staged_app,
        &plan.backup_app,
        &plan.mount_dir,
        &plan.script_path,
    ] {
        assert!(path.starts_with(&plan.staging_dir), "{path:?}");
    }

    let quoted = Path::new("/opt/it's here/Mundus Manager.app");
    assert!(MacInstallPlan::for_app(quoted).is_err());
}

#[test]
fn read_bundle_id_parses_the_packaged_plist() {
    let dir = tempfile::tempdir().unwrap();
    let contents = dir.path().join("Mundus Manager.app/Contents");
    std::fs::create_dir_all(&contents).unwrap();
    std::fs::write(
        contents.join("Info.plist"),
        r#"<?xml version="1.0"?><plist version="1.0"><dict>
          <key>CFBundleName</key><string>Mundus Manager</string>
          <key>CFBundleIdentifier</key><string>com.kazui.mundus.manager</string>
        </dict></plist>"#,
    )
    .unwrap();
    assert_eq!(
        read_bundle_id(&dir.path().join("Mundus Manager.app")).unwrap(),
        "com.kazui.mundus.manager"
    );
}

#[test]
fn helper_script_quits_swaps_relaunches_and_cleans_up() {
    let plan = MacInstallPlan::for_app(Path::new("/Applications/Mundus Manager.app")).unwrap();
    let script = helper_script(&plan, "com.kazui.mundus.manager");
    // Ordering matters: graceful engine shutdown → wait/kill → swap →
    // relaunch → cleanup. Graceful goes through the engine's own control
    // channel, never osascript (Automation consent prompt).
    assert!(!script.contains("osascript"));
    let quit = script.find("mundus-engine\" --shutdown").unwrap();
    let kill = script.find("kill -9").unwrap();
    let swap = script.find("mv \"$APP\" \"$BACKUP\"").unwrap();
    let relaunch = script.find("open \"$APP\"").unwrap();
    let cleanup = script.find("rm -rf \"$STAGING\"").unwrap();
    assert!(quit < kill && kill < swap && swap < relaunch && relaunch < cleanup);
    assert!(
        script.contains("'$APP/Contents/MacOS/'") || script.contains("\"$APP/Contents/MacOS/\"")
    );
    assert!(script.contains("/Applications/Mundus Manager.app"));
    // Rollback: a failed swap restores the previous bundle.
    assert!(script.contains("mv \"$BACKUP\" \"$APP\""));
}

/// The generated helper actually performs the swap on a real filesystem —
/// run with stub mac tools on any unix host.
#[cfg(unix)]
#[test]
fn helper_script_swaps_the_bundle_and_relaunches() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let parent = dir.path().join("Applications");
    std::fs::create_dir(&parent).unwrap();
    let app = parent.join("Mundus Manager.app");
    std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    std::fs::write(app.join("Contents/MacOS/mundus-engine"), b"old").unwrap();

    let plan = MacInstallPlan::for_app(&app).unwrap();
    std::fs::create_dir_all(&plan.staging_dir).unwrap();
    std::fs::create_dir_all(plan.staged_app.join("Contents/MacOS")).unwrap();
    std::fs::write(plan.staged_app.join("Contents/MacOS/mundus-engine"), b"new").unwrap();
    std::fs::write(plan.staged_app.join("NEW_MARKER"), b"1").unwrap();

    // Stubs: no running processes, relaunch recorded.
    let stubs = dir.path().join("stub-bin");
    std::fs::create_dir(&stubs).unwrap();
    for (name, body) in [
        ("pgrep", "#!/bin/sh\nexit 1\n"),
        ("xattr", "#!/bin/sh\nexit 0\n"),
        ("open", "#!/bin/sh\n: > \"$STUB_OPEN_MARKER\"\nexit 0\n"),
    ] {
        let path = stubs.join(name);
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let marker = dir.path().join("opened");
    let script = helper_script(&plan, "com.kazui.mundus.manager");
    let status = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(&script)
        .env(
            "PATH",
            format!("{}:{}", stubs.display(), std::env::var("PATH").unwrap()),
        )
        .env("STUB_OPEN_MARKER", &marker)
        .status()
        .unwrap();
    assert!(status.success());

    assert_eq!(
        std::fs::read(app.join("Contents/MacOS/mundus-engine")).unwrap(),
        b"new"
    );
    assert!(marker.exists(), "helper did not relaunch the app");
    assert!(!plan.staging_dir.exists(), "staging not cleaned up");
    let leftovers: Vec<_> = std::fs::read_dir(&parent)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}
