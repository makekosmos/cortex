// macOS arm of the updater service (KOS-377): feed fallback for a stale
// win-only manifest, and the full DMG apply with stub mac tools.
use super::tests::{hash, manifest_json};
use super::*;
use httpmock::MockServer;
use std::path::Path;

#[tokio::test]
async fn mac_check_uses_legacy_feed_when_manifest_lacks_the_mac_entry() {
    // KOS-377: a stale win-only manifest.json (CDN lag right after publish)
    // must not hide a real mac update while latest-mac.yml names the DMG.
    let body = b"dmg payload".repeat(40);
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/manifest.json");
            then.status(200).body(manifest_json(
                "99.0.0",
                r#""win":{"file":"Mundus-Setup-99.0.0.exe","size":1,"sha512":"AAA"}"#,
            ));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/latest-mac.yml");
            then.status(200).body(format!(
                "version: 99.0.0\nfiles:\n  - url: Mundus-99.0.0.dmg\n    sha512: {}\n    \
                 size: {}\n",
                hash(&body),
                body.len()
            ));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/Mundus-99.0.0.dmg");
            then.status(200).body(body.clone());
        })
        .await;
    let dir = tempfile::tempdir().unwrap();
    let service = UpdaterService::try_with_feed_base(
        dir.path().to_path_buf(),
        server.base_url(),
        "0.5.0".into(),
        true,
        "mac",
    )
    .unwrap();

    assert_eq!(service.check().await["state"], "available");
    assert_eq!(service.status()["newVersion"], "99.0.0");
    for _ in 0..200 {
        if service.status()["state"] == "downloaded" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(service.status()["state"], "downloaded");
    assert_eq!(
        tokio::fs::read(dir.path().join("updates/Mundus-99.0.0.dmg"))
            .await
            .unwrap(),
        body
    );
}

#[cfg(unix)]
fn write_stub(dir: &Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A .app skeleton: Info.plist with the given bundle id plus `extra` files.
#[cfg(unix)]
fn write_app(root: &Path, bundle_id: &str, extra: &[(&str, &str)]) {
    let contents = root.join("Contents");
    std::fs::create_dir_all(contents.join("MacOS")).unwrap();
    std::fs::write(
        contents.join("Info.plist"),
        format!(
            "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict>\
             <key>CFBundleIdentifier</key><string>{bundle_id}</string>\
             <key>CFBundleExecutable</key><string>Mundus Manager</string></dict></plist>"
        ),
    )
    .unwrap();
    std::fs::write(contents.join("MacOS").join("mundus-engine"), b"engine").unwrap();
    for (name, body) in extra {
        std::fs::write(root.join(name), body).unwrap();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn mac_install_stages_the_dmg_app_over_the_running_bundle_and_relaunches() {
    // KOS-377: the full mac arm — check finds the DMG, downloads it, install
    // mounts it, stages the .app next to the running bundle, and the detached
    // helper swaps it in and relaunches. The mac tools (hdiutil, ditto,
    // whole flow runs on any unix host.
    let body = b"dmg payload".repeat(40);
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/manifest.json");
            then.status(200).body(manifest_json(
                "99.0.0",
                &format!(
                    r#""mac":{{"file":"Mundus-99.0.0.dmg","size":{},"sha512":"{}"}}"#,
                    body.len(),
                    hash(&body)
                ),
            ));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/Mundus-99.0.0.dmg");
            then.status(200).body(body.clone());
        })
        .await;

    let dir = tempfile::tempdir().unwrap();
    // Stub mac tools. hdiutil "mounts" by copying the fixture app into the
    // requested mountpoint; ditto copies recursively; the rest are no-ops
    // except `open`, which records the relaunch.
    let stubs = dir.path().join("stub-bin");
    std::fs::create_dir(&stubs).unwrap();
    let fixture_app = dir.path().join("fixture/Mundus Manager.app");
    write_app(
        &fixture_app,
        "com.kazui.mundus.manager",
        &[("NEW_VERSION_MARKER", "99.0.0")],
    );
    let open_marker = dir.path().join("opened");
    write_stub(
        &stubs,
        "hdiutil",
        concat!(
            "#!/bin/sh\n",
            "if [ \"$1\" = \"attach\" ]; then\n",
            "  mnt=\"\"; prev=\"\"\n",
            "  for a in \"$@\"; do [ \"$prev\" = \"-mountpoint\" ] && mnt=\"$a\"; prev=\"$a\"; done\n",
            "  mkdir -p \"$mnt\" && cp -R \"$STUB_MOUNT_APP\" \"$mnt/\"\n",
            "else\n  exit 0\nfi\n"
        ),
    );
    write_stub(&stubs, "ditto", "#!/bin/sh\ncp -R \"$1\" \"$2\"\n");
    write_stub(&stubs, "pgrep", "#!/bin/sh\nexit 1\n");
    write_stub(&stubs, "xattr", "#!/bin/sh\nexit 0\n");
    write_stub(
        &stubs,
        "open",
        "#!/bin/sh\n: > \"$STUB_OPEN_MARKER\"\nexit 0\n",
    );

    // The "installed" app the fake Engine runs from.
    let installed_parent = dir.path().join("Applications");
    std::fs::create_dir(&installed_parent).unwrap();
    let installed_app = installed_parent.join("Mundus Manager.app");
    write_app(&installed_app, "com.kazui.mundus.manager", &[]);

    std::env::set_var(
        "PATH",
        format!("{}:{}", stubs.display(), std::env::var("PATH").unwrap()),
    );
    std::env::set_var("STUB_MOUNT_APP", &fixture_app);
    std::env::set_var("STUB_OPEN_MARKER", &open_marker);
    std::env::set_var("MUNDUS_UPDATER_APP_BUNDLE", &installed_app);

    let service = UpdaterService::try_with_feed_base(
        dir.path().to_path_buf(),
        server.base_url(),
        "0.5.0".into(),
        true,
        "mac",
    )
    .unwrap();
    assert_eq!(service.check().await["state"], "available");
    for _ in 0..200 {
        if service.status()["state"] == "downloaded" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(service.status()["state"], "downloaded");

    service.install();

    for _ in 0..500 {
        if open_marker.exists() {
            break;
        }
        if service.status()["state"] == "error" {
            panic!("install failed: {}", service.status()["message"]);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(open_marker.exists(), "helper never relaunched the app");
    assert_eq!(service.status()["state"], "downloaded");
    // The new bundle replaced the installed one; the backup is cleaned up.
    assert_eq!(
        std::fs::read_to_string(installed_app.join("NEW_VERSION_MARKER")).unwrap(),
        "99.0.0"
    );
    // `open` is recorded before the helper's final `rm -rf` — wait for it.
    for _ in 0..200 {
        let done = std::fs::read_dir(&installed_parent).unwrap().all(|e| {
            !e.unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".mundus-update")
        });
        if done {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let leftovers: Vec<_> = std::fs::read_dir(&installed_parent)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".mundus-update") || n.ends_with(".old.app"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "leftover update artifacts: {leftovers:?}"
    );
}
