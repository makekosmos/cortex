#[test]
fn package_summary_exposes_a_verified_icon_path() {
    let dir = tempdir().expect("temp dir");
    let mut manifest = manifest();
    manifest.icon = Some("icon.ico".into());
    let expected = VersionedManifest::V2(manifest);
    let (archive, hash, size) = archive_with_versioned_manifest(dir.path(), &expected);
    let store = PackageStore::new(dir.path().join("packages")).expect("store");
    let package = store
        .install_versioned(&archive, size, &hash, &expected, 1)
        .expect("install");

    let listed = summary(package, None, &store);
    assert_eq!(listed.name, "Demo");
    let icon = listed.icon_path.expect("icon path");
    assert!(icon.ends_with("icon.ico"));
    assert!(!icon.starts_with(r"\\?\"));
}

#[test]
fn summaries_contain_no_sensitive_package_fields() {
    let dir = tempdir().expect("tempdir");
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let json = serde_json::to_string(&service.list().expect("list")).expect("json");
    for forbidden in [
        "sha256",
        "hash",
        "entrypoint",
        "manifest",
        "signature",
        "public_key",
        "path",
    ] {
        assert!(!json.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn disclosure_projects_the_catalog_manifest_contract() {
    use crate::package_manifest::{
        DataAccessRule, DataAction, FieldAccess, ManifestMapping, MappingDirection, MappingFidelity,
    };
    let dir = tempdir().expect("tempdir");
    let service = PackageService::open_for_test(dir.path()).expect("service");
    let mut package_manifest = manifest();
    package_manifest.permissions = vec![PermissionRequest {
        capability: "network".into(),
        scopes: vec!["https://api.example.com/".into()],
    }];
    package_manifest.data.access = vec![DataAccessRule {
        type_id: "core.note".into(),
        versions: "*".into(),
        actions: vec![DataAction::Read, DataAction::Update],
        fields: FieldAccess {
            read: vec!["props.description".into()],
            write: vec!["props.extensions".into()],
        },
        relations: None,
    }];
    package_manifest.data.mappings = vec![ManifestMapping {
        type_id: "core.note".into(),
        versions: "*".into(),
        direction: MappingDirection::Export,
        fidelity: MappingFidelity::Lossy,
    }];
    let mut doc = catalog(1, "a".repeat(64), 1, "2030-01-01T00:00:00Z");
    doc.packages[0].manifest = VersionedManifest::V2(package_manifest);
    let bytes = document_bytes(&doc);
    service.apply_catalog(&bytes).expect("catalog");

    let disclosure = service
        .disclosure("com.kosmos.demo", "1.0.0")
        .expect("disclosure");
    assert_eq!(disclosure.name, "Demo");
    assert_eq!(disclosure.capabilities.len(), 1);
    assert_eq!(disclosure.capabilities[0].capability, "network");
    assert_eq!(
        disclosure.capabilities[0].scopes,
        vec!["https://api.example.com/"]
    );
    assert_eq!(disclosure.data.len(), 1);
    assert_eq!(
        disclosure.data[0].actions,
        vec![DataAction::Read, DataAction::Update]
    );
    assert_eq!(disclosure.data[0].fields_read, vec!["props.description"]);
    assert_eq!(disclosure.data[0].fields_write, vec!["props.extensions"]);
    assert_eq!(disclosure.mappings.len(), 1);
    assert_eq!(disclosure.mappings[0].direction, MappingDirection::Export);
    assert_eq!(
        disclosure.volume_scopes,
        Some(vec![DisclosureVolumeScope {
            type_id: "core.note".into(),
            type_version: "1.0.0".into(),
            fields: vec!["props.description".into()],
        }])
    );
    assert!(service.disclosure("com.kosmos.demo", "9.9.9").is_err());
    assert!(service.disclosure("com.kosmos.missing", "1.0.0").is_err());
    assert!(service.disclosure("", "1.0.0").is_err());
}

#[test]
fn disclosure_falls_back_to_the_verified_installed_manifest() {
    let dir = tempdir().expect("tempdir");
    let (service, _, _) = enabled_app_service(dir.path());
    let disclosure = service
        .disclosure("com.kosmos.demo", "1.0.0")
        .expect("installed disclosure");
    assert_eq!(disclosure.id, "com.kosmos.demo");
    assert_eq!(disclosure.version, "1.0.0");
}
