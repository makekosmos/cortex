use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn installed_worker_is_executable_without_trusting_zip_permissions() {
    let dir = tempdir().unwrap();
    let store = PackageStore::new(dir.path()).unwrap();
    let VersionedManifest::V2(mut manifest) = manifest_v2() else {
        panic!("expected v2 manifest")
    };
    manifest.kind = PackageKind::Source;
    manifest.entrypoint = "worker.exe".into();
    manifest.targets[0].runtime = crate::package_manifest::TargetRuntime::Worker;
    manifest.targets[0].os = vec![crate::package_manifest::TargetOs::current()];
    let manifest = VersionedManifest::V2(manifest);
    let path = dir.path().join("worker.kspkg");
    let mut zip = ZipWriter::new(fs::File::create(&path).unwrap());
    // Archive headers deliberately mark every member as non-executable.
    let options = FileOptions::default().unix_permissions(0o644);
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(&serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    zip.start_file("worker.exe", options).unwrap();
    zip.write_all(b"#!/bin/sh\nprintf 'worker-started\\n'\n")
        .unwrap();
    zip.start_file("data.txt", options).unwrap();
    zip.write_all(b"not executable").unwrap();
    zip.finish().unwrap();
    let bytes = fs::read(&path).unwrap();
    let installed = store
        .install_versioned(&path, bytes.len() as u64, &hex_hash(&bytes), &manifest, 1)
        .unwrap();
    let worker = store.immutable_entrypoint(&installed).unwrap();
    let output = std::process::Command::new(&worker).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"worker-started\n");
    assert_eq!(
        fs::metadata(&worker).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in ["manifest.json", "data.txt"] {
        let mode = fs::metadata(worker.parent().unwrap().join(name))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0, "non-worker {name} must not be executable");
    }
}
