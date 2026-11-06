use super::*;
#[test]
fn published_mapping_direction_accepts_kebab_case_without_changing_wire_output() {
    for spelling in ["bidirectional-sync", "bidirectionalSync"] {
        let direction: MappingDirection =
            serde_json::from_value(serde_json::json!(spelling)).unwrap();
        assert_eq!(direction, MappingDirection::BidirectionalSync);
        assert_eq!(
            serde_json::to_value(direction).unwrap(),
            "bidirectionalSync"
        );
    }
    assert!(
        serde_json::from_value::<MappingDirection>(serde_json::json!("unsupported-sync")).is_err()
    );
}

fn valid() -> PackageManifest {
    PackageManifest {
        schema_version: 1,
        id: "com.kosmos.demo".into(),
        name: "Demo".into(),
        version: "1.2.3".into(),
        kind: PackageKind::App,
        engine_api: ">=1.0.0".into(),
        entrypoint: "dist/index.html".into(),
        publisher: "kosmos".into(),
        permissions: vec![],
    }
}
#[test]
fn v1_validation_and_unknown_fields() {
    let m = valid();
    assert!(m.validate().is_ok());
    assert!(serde_json::from_str::<PackageManifest>(concat!(
        r#"{"schema_version":1,"id":"x","name":"x","version":"1.0.0","kind":"app","#,
        r#""engine_api":"*","entrypoint":"x","publisher":"kosmos","nope":1}"#,
    ))
    .is_err());
}
#[test]
fn v2_strict_contract_and_dependencies() {
    let s = concat!(
        r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.2.3","#,
        r#""kind":"app","engine_api":"*","entrypoint":"index.html","icon":"icon.ico","#,
        r#""publisher":"kosmos","targets":[{"runtime":"kosmos-host","os":["linux"]}],"#,
        r#""data":{"access":[],"defines":[],"mappings":[]}}"#
    );
    let VersionedManifest::V2(m) = PackageManifest::parse(s).unwrap() else {
        panic!()
    };
    assert!(m.validate().is_ok());
    assert_eq!(m.icon.as_deref(), Some("icon.ico"));
    let d = s.replace("\"mappings\":[]", "\"mappings\":[],\"nope\":1");
    assert!(PackageManifest::parse(&d).is_err());
}

#[test]
fn v2_rejects_unsafe_or_non_windows_icon_paths() {
    for icon in ["../icon.ico", "icon.svg", "icon.ico/extra"] {
        let input = format!(
            concat!(
                r#"{{"schema_version":2,"id":"com.kosmos.demo","#,
                r#""name":"Demo","version":"1.2.3","#,
                r#""kind":"app","engine_api":"*","entrypoint":"index.html","icon":"{icon}","#,
                r#""publisher":"kosmos","#,
                r#""targets":[{{"runtime":"kosmos-host","os":["linux"]}}],"#,
                r#""data":{{"access":[],"defines":[],"mappings":[]}}}}"#
            ),
            icon = icon
        );
        assert!(PackageManifest::parse(&input).is_err(), "{icon}");
    }
}

#[test]
fn v2_rejects_duplicate_targets_and_overlong_requirements() {
    let base = concat!(
        r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.2.3","#,
        r#""kind":"app","engine_api":"*","entrypoint":"index.html","#,
        r#""publisher":"kosmos","targets":[{"runtime":"kosmos-host","os":["linux"]},"#,
        r#"{"runtime":"kosmos-host","os":["linux"]}],"data":{"access":[],"defines":[],"#,
        r#""mappings":[]}}"#
    );
    assert!(PackageManifest::parse(base).is_err());
    let long = base.replace(
        "\"access\":[]",
        &format!(
            "\"access\":[{{\"type\":\"core.note\",\"versions\":\"{}\",\
                    \"actions\":[\"read\"],\"fields\":{{\"read\":[],\"write\":[]}}}}]",
            "x".repeat(129)
        ),
    );
    assert!(PackageManifest::parse(&long).is_err());
}

#[test]
fn v2_rejects_duplicate_mappings_and_long_namespace_suffix() {
    let s = concat!(
        r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.2.3","#,
        r#""kind":"app","engine_api":"*","entrypoint":"index.html","#,
        r#""publisher":"kosmos","targets":[{"runtime":"kosmos-host","os":["linux"]}],"#,
        r#""data":{"access":[],"defines":[],"mappings":[{"type":"core.note","#,
        r#""versions":"*","direction":"import","fidelity":"native"},"#,
        r#"{"type":"core.note","versions":"*","direction":"export","#,
        r#""fidelity":"native"}]}}"#
    );
    assert!(PackageManifest::parse(s).is_err());
    let long = s.replace(
        "\"mappings\":[",
        &format!(
            concat!(
                "\"defines\":[{{\"type\":\"com.kosmos.demo.{}\",\"version\":\"1.0.0\",",
                "\"schema\":\"schema.json\"}}],\"mappings\":["
            ),
            "x".repeat(65)
        ),
    );
    assert!(PackageManifest::parse(&long).is_err());
}

#[test]
fn schema_versions_are_not_shared() {
    let mut wrong = valid();
    wrong.schema_version = 2;
    assert!(wrong.validate().is_err());
}

#[test]
fn v2_store_metadata_validates() {
    let s = concat!(
        r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.2.3","#,
        r#""kind":"source","engine_api":"*","entrypoint":"w.exe","#,
        r#""publisher":"kosmos","targets":[{"runtime":"worker","os":["windows"]}],"#,
        r#""data":{"access":[],"defines":[],"mappings":[]},"#,
        r#""store":{"description":"d","categories":["integrations"],"#,
        r#""connects_to":"external.demo","data_compatibility":[{"type":"core.note","#,
        r#""versions":"*","roles":["import"],"via":"com.kosmos.demo","#,
        r#""fidelity":"lossless"}]}}"#
    );
    assert!(PackageManifest::parse(s).is_ok());
    for bad in [
        r#""connects_to":"not-external""#,
        r#""categories":["UPPER"]"#,
        concat!(
            r#""data_compatibility":[{"type":"core.note","versions":"*","#,
            r#""roles":[],"via":"com.kosmos.demo","fidelity":"lossless"}]"#
        ),
    ] {
        let broken = s.replacen(
            match bad {
                r#""connects_to":"not-external""# => r#""connects_to":"external.demo""#,
                r#""categories":["UPPER"]"# => r#""categories":["integrations"]"#,
                _ => r#""roles":["import"]"#,
            },
            bad,
            1,
        );
        assert!(PackageManifest::parse(&broken).is_err(), "{bad}");
    }
}

#[test]
fn worker_entrypoint_selects_the_current_target_only() {
    let input = concat!(
        r#"{"schema_version":2,"id":"com.kosmos.demo","name":"Demo","version":"1.2.3","#,
        r#""kind":"app","engine_api":"*","entrypoint":"index.html","#,
        r#""publisher":"kosmos","targets":[{"runtime":"kosmos-host","os":["windows"]},"#,
        r#"{"runtime":"worker","os":["windows"],"arch":["x86_64"],"#,
        r#""entrypoint":"worker-windows.exe"},{"runtime":"worker","os":["linux"],"#,
        r#""arch":["x86_64"],"entrypoint":"worker-linux.exe"},{"runtime":"worker","#,
        r#""os":["macos"],"entrypoint":"worker-macos.exe"}],"data":{"access":[],"#,
        r#""defines":[],"mappings":[]}}"#
    );
    let manifest = PackageManifest::parse(input).unwrap();
    let expected = if cfg!(windows) {
        Some("worker-windows.exe")
    } else if cfg!(target_os = "linux") {
        Some("worker-linux.exe")
    } else if cfg!(target_os = "macos") {
        Some("worker-macos.exe")
    } else {
        None
    };
    assert_eq!(manifest.worker_entrypoint(), expected);
    let VersionedManifest::V2(manifest) = manifest else {
        panic!()
    };
    assert!(manifest.supports_current_platform());
    assert_eq!(manifest.declared_worker_entrypoints().len(), 3);
}
