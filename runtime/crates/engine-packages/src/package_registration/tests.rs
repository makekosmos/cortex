use super::*;
use crate::package_manifest::PackageManifest;
use tempfile::tempdir;

fn manifest() -> ManifestV2 {
    let raw = concat!(
        r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.0.0","#,
        r#""kind":"app","engine_api":"*","entrypoint":"index.html","#,
        r#""publisher":"kosmos","targets":[{"runtime":"standalone","os":["linux"]}],"#,
        r#""data":{"access":[],"defines":[{"type":"com.kosmos.demo.note","#,
        r#""version":"1.0.0","schema":"schema.json","content_contract":"content.json","#,
        r#""relations":"relations.json"}],"mappings":[]}}"#
    );
    let crate::package_manifest::VersionedManifest::V2(m) = PackageManifest::parse(raw).unwrap()
    else {
        panic!()
    };
    m
}

#[test]
fn same_version_contract_changes_conflict_without_mutating_persisted_definition() {
    let dir = tempdir().unwrap();
    let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
    let mut docs = BTreeMap::new();
    docs.insert("schema.json".into(), br#"{}"#.to_vec());
    docs.insert("content.json".into(), br#"{"body":"text"}"#.to_vec());
    docs.insert("relations.json".into(), br#"["project"]"#.to_vec());
    registry.register_manifest(&manifest(), &docs).unwrap();
    let before = registry.definitions().unwrap()[0].schema_hash.clone();
    let mut changed_contract = docs.clone();
    changed_contract.insert("content.json".into(), br#"{"body":"rich_text"}"#.to_vec());
    assert!(matches!(
        registry.register_manifest(&manifest(), &changed_contract),
        Err(RegistrationError::Conflict)
    ));
    assert_eq!(registry.definitions().unwrap()[0].schema_hash, before);
    registry.register_manifest(&manifest(), &docs).unwrap();
    registry.uninstall("com.kosmos.demo").unwrap();
    assert_eq!(registry.definitions().unwrap().len(), 1);
}

#[test]
fn conflicting_reinstall_is_atomic_and_foreign_namespace_rejected() {
    let dir = tempdir().unwrap();
    let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
    let mut docs = BTreeMap::new();
    docs.insert("schema.json".into(), br#"{}"#.to_vec());
    docs.insert("content.json".into(), br#"{"body":"text"}"#.to_vec());
    docs.insert("relations.json".into(), br#"["project"]"#.to_vec());
    registry.register_manifest(&manifest(), &docs).unwrap();
    let mut changed = docs.clone();
    changed.insert("schema.json".into(), br#"{"x":1}"#.to_vec());
    assert!(matches!(
        registry.register_manifest(&manifest(), &changed),
        Err(RegistrationError::Conflict)
    ));
    let foreign = RegisteredDefinition {
        type_id: "com.kosmos.demo.note".into(),
        version: "1.0.0".into(),
        owner_id: "com.kosmos.foreign".into(),
        schema_hash: "foreign".into(),
        content_contract_hash: "foreign".into(),
        relations_hash: "foreign".into(),
        fields: vec![],
        relations: vec![],
        schema_json: "{}".into(),
        content_contract_json: "{}".into(),
        relations_json: "[]".into(),
    };
    fs::write(
        dir.path().join("definitions.json"),
        serde_json::to_vec(&RegistryState {
            definitions: [("com.kosmos.demo.note@1.0.0".into(), foreign)]
                .into_iter()
                .collect(),
        })
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        registry.register_manifest(&manifest(), &docs),
        Err(RegistrationError::Conflict)
    ));
    assert_eq!(registry.definitions().unwrap().len(), 1);
}

#[test]
fn content_and_relations_changes_change_digest_independently() {
    let dir = tempdir().unwrap();
    let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
    let mut docs = BTreeMap::from([
        ("schema.json".into(), br#"{}"#.to_vec()),
        ("content.json".into(), br#"{"kind":"text"}"#.to_vec()),
        ("relations.json".into(), br#"["project"]"#.to_vec()),
    ]);
    registry.register_manifest(&manifest(), &docs).unwrap();
    let first = registry.definitions().unwrap().pop().unwrap();
    docs.insert("relations.json".into(), br#"["area"]"#.to_vec());
    assert!(matches!(
        registry.register_manifest(&manifest(), &docs),
        Err(RegistrationError::Conflict)
    ));
    let second = registry.definitions().unwrap().pop().unwrap();
    assert_eq!(first, second);
    docs.insert("content.json".into(), br#"{"kind":"rich_text"}"#.to_vec());
    assert!(matches!(
        registry.register_manifest(&manifest(), &docs),
        Err(RegistrationError::Conflict)
    ));
    assert_eq!(second, registry.definitions().unwrap().pop().unwrap());
}

#[test]
fn missing_or_malformed_referenced_contract_fails_without_persisting() {
    let dir = tempdir().unwrap();
    let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
    let docs = BTreeMap::from([(String::from("schema.json"), br#"{}"#.to_vec())]);
    assert!(matches!(
        registry.register_manifest(&manifest(), &docs),
        Err(RegistrationError::Conflict)
    ));
    assert!(registry.definitions().unwrap().is_empty());
    let mut malformed = docs;
    malformed.insert("content.json".into(), b"{".to_vec());
    malformed.insert("relations.json".into(), br#"[]"#.to_vec());
    assert!(matches!(
        registry.register_manifest(&manifest(), &malformed),
        Err(RegistrationError::Conflict)
    ));
    assert!(registry.definitions().unwrap().is_empty());
}

#[test]
fn absent_optional_contracts_remain_valid_and_legacy_records_deserialize() {
    let dir = tempdir().unwrap();
    let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
    let raw = concat!(
        r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.0.0","#,
        r#""kind":"app","engine_api":"*","entrypoint":"index.html","publisher":"kosmos","#,
        r#""targets":[{"runtime":"standalone","os":["linux"]}],"data":{"access":[],"#,
        concat!(
            r#""defines":[{"type":"com.kosmos.demo.note","version":"1.0.0","#,
            r#""schema":"schema.json"}],"#,
        ),
        r#""mappings":[]}}"#,
    );
    let crate::package_manifest::VersionedManifest::V2(manifest) =
        PackageManifest::parse(raw).unwrap()
    else {
        panic!()
    };
    registry
        .register_manifest(
            &manifest,
            &BTreeMap::from([(String::from("schema.json"), br#"{}"#.to_vec())]),
        )
        .unwrap();
    assert_eq!(registry.definitions().unwrap().len(), 1);
    let legacy = concat!(
        r#"{"definitions":{"com.kosmos.demo.note@1.0.0":{"type_id":"com.kosmos.demo.note","#,
        r#""version":"1.0.0","owner_id":"com.kosmos.demo","schema_hash":"old"}}}"#,
    )
    .as_bytes();
    let parsed: RegistryState = serde_json::from_slice(legacy).unwrap();
    assert_eq!(
        parsed.definitions["com.kosmos.demo.note@1.0.0"].content_contract_hash,
        ""
    );
}

#[test]
fn legacy_definition_is_replaced_only_by_verified_archive_documents() {
    let dir = tempdir().unwrap();
    let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
    let legacy = concat!(
        r#"{"definitions":{"com.kosmos.demo.note@1.0.0":{"type_id":"com.kosmos.demo.note","#,
        r#""version":"1.0.0","owner_id":"com.kosmos.demo","schema_hash":"old"}}}"#,
    )
    .as_bytes();
    fs::write(dir.path().join("definitions.json"), legacy).unwrap();
    let mut docs = BTreeMap::new();
    docs.insert("schema.json".into(), br#"{"type":"object"}"#.to_vec());
    docs.insert("content.json".into(), br#"{}"#.to_vec());
    docs.insert("relations.json".into(), br#"[]"#.to_vec());
    registry.register_manifest(&manifest(), &docs).unwrap();
    assert_eq!(
        registry.type_registrations().unwrap()[0].schema_json,
        r#"{"type":"object"}"#
    );
}

#[test]
fn incomplete_legacy_definition_is_not_replayed_without_archive() {
    let dir = tempdir().unwrap();
    let registry = PackageRegistrationRegistry::open(dir.path()).unwrap();
    let legacy = concat!(
        r#"{"definitions":{"com.kosmos.demo.note@1.0.0":{"type_id":"com.kosmos.demo.note","#,
        r#""version":"1.0.0","owner_id":"com.kosmos.demo","schema_hash":"old"}}}"#,
    )
    .as_bytes();
    fs::write(dir.path().join("definitions.json"), legacy).unwrap();
    assert!(registry.type_registrations().unwrap().is_empty());
}
