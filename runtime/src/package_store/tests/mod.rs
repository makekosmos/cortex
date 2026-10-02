mod install;
mod lifecycle;
mod verify;

use super::*;
use tempfile::tempdir;
use zip::{write::FileOptions, ZipWriter};
fn manifest() -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: "com.kosmos.demo".into(),
        name: "Demo".into(),
        version: "1.0.0".into(),
        kind: PackageKind::App,
        engine_api: ">=1.0.0".into(),
        entrypoint: "index.html".into(),
        publisher: "kosmos".into(),
        permissions: vec![],
    }
}
fn archive<M: Serialize>(path: &Path, manifest: &M, entry: &str) {
    let f = fs::File::create(path).unwrap();
    let mut z = ZipWriter::new(f);
    let o = FileOptions::default();
    z.start_file("manifest.json", o).unwrap();
    z.write_all(&serde_json::to_vec(manifest).unwrap()).unwrap();
    z.start_file(entry, o).unwrap();
    z.write_all(b"ok").unwrap();
    z.finish().unwrap();
}
fn manifest_v2() -> VersionedManifest {
    PackageManifest::parse(
            r#"{"schema_version":2,"id":"com.kosmos.v2-demo","name":"V2 Demo","version":"2.0.0","kind":"app","engine_api":">=1.0.0","entrypoint":"index.html","publisher":"kosmos","permissions":[],"targets":[{"runtime":"standalone","os":["windows"]}],"data":{"access":[],"defines":[],"mappings":[]}}"#,
        )
        .unwrap()
}
fn archive_versioned(path: &Path, manifest: &VersionedManifest, entry: &str) {
    let f = fs::File::create(path).unwrap();
    let mut z = ZipWriter::new(f);
    let o = FileOptions::default();
    z.start_file("manifest.json", o).unwrap();
    z.write_all(&serde_json::to_vec(manifest).unwrap()).unwrap();
    z.start_file(entry, o).unwrap();
    z.write_all(b"ok").unwrap();
    z.finish().unwrap();
}
fn central_entry(bytes: &[u8], expected_name: &[u8]) -> usize {
    bytes
        .windows(4)
        .enumerate()
        .find_map(|(offset, signature)| {
            if signature != b"PK\x01\x02" {
                return None;
            }
            let name_len = u16::from_le_bytes([bytes[offset + 28], bytes[offset + 29]]) as usize;
            (&bytes[offset + 46..offset + 46 + name_len] == expected_name).then_some(offset)
        })
        .unwrap()
}
use std::io::Write;
fn install_worker(dir: &Path, store: &PackageStore) -> (InstalledPackage, PathBuf) {
    let VersionedManifest::V2(mut manifest) = manifest_v2() else {
        panic!("expected v2 manifest")
    };
    manifest.kind = PackageKind::Source;
    manifest.entrypoint = "worker.exe".into();
    manifest.targets[0].runtime = crate::package_manifest::TargetRuntime::Worker;
    manifest.targets[0].os = vec![
        crate::package_manifest::TargetOs::Windows,
        crate::package_manifest::TargetOs::Macos,
        crate::package_manifest::TargetOs::Linux,
    ];
    manifest.targets[0].entrypoint = Some("worker.exe".into());
    let archive_path = dir.join("worker.kspkg");
    archive(&archive_path, &manifest, "worker.exe");
    let bytes = fs::read(&archive_path).unwrap();
    let hash = hex_hash(&bytes);
    let installed = store
        .install_versioned(
            &archive_path,
            bytes.len() as u64,
            &hash,
            &VersionedManifest::V2(manifest),
            1,
        )
        .unwrap();
    let entrypoint = store.immutable_entrypoint(&installed).unwrap();
    (installed, entrypoint)
}

fn blob_path(dir: &Path, installed: &InstalledPackage) -> PathBuf {
    dir.join("store")
        .join("blobs")
        .join(format!("{}.kspkg", installed.hash))
}
