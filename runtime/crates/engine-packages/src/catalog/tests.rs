use super::*;

fn document_json() -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "sequence": 1,
        "issued_at": "2026-10-03T00:00:00Z",
        "expires_at": "2027-10-03T00:00:00Z",
        "packages": [{
            "manifest": {
                "schema_version": 2, "id": "com.kosmos.demo", "name": "Demo",
                "version": "1.0.0", "kind": "source", "engine_api": "*",
                "entrypoint": "w.exe", "publisher": "kosmos",
                "targets": [{"runtime": "worker", "os": ["windows"]}],
                "data": {"access": [], "defines": [], "mappings": []},
                "store": {
                    "categories": ["integrations"],
                    "connects_to": "external.demo",
                    "data_compatibility": [{
                        "type": "com.kosmos.note", "versions": "*",
                        "roles": ["import"], "via": "external.demo",
                        "fidelity": "lossless"
                    }]
                }
            },
            "archives": [{
                "os": "windows",
                "arch": "x86_64",
                "url": "https://packages.kosmos.dev/demo.kspkg",
                "sha256": "a".repeat(64),
                "size": 123
            }]
        }],
        "external_apps": [{
            "id": "external.demo", "name": "Demo", "publisher": "Demo Inc",
            "publisher_tier": "verified", "description": "d",
            "categories": ["education"], "platforms": ["windows"],
            "official_url": "https://demo.example", "icon_url": null,
            "data_compatibility": []
        }],
        "revoked": []
    })
}

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-04T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn a_real_shaped_catalog_parses_and_lists() {
    let bytes = serde_json::to_vec(&document_json()).unwrap();
    let document = CatalogDocument::parse(&bytes, now(), 0).unwrap();
    assert_eq!(document.sequence, 1);
    let listings = document.listings();
    assert_eq!(listings.len(), 2);
    let package = listings.iter().find(|l| l.id == "com.kosmos.demo").unwrap();
    assert_eq!(package.kind, ListingKind::Integration);
    assert_eq!(
        package.distribution,
        Distribution::Integration {
            package_id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            connects_to: "external.demo".into(),
        }
    );
    assert!(document
        .external_url("external.demo")
        .is_some_and(|url| url == "https://demo.example"));
}

#[test]
fn windows_only_catalog_with_published_sync_mapping_parses_on_every_host() {
    let mut value = document_json();
    value["packages"][0]["manifest"]["data"]["mappings"] = serde_json::json!([{
        "type": "com.mundus.note", "versions": "^1.0.0",
        "direction": "bidirectional-sync", "fidelity": "lossless"
    }]);
    let bytes = serde_json::to_vec(&value).unwrap();
    let document = CatalogDocument::parse(&bytes, now(), 0).unwrap();
    assert_eq!(document.listings().len(), 2);
    if TargetOs::current() != TargetOs::Windows {
        assert!(document.packages[0].archive().is_none());
    }
    assert!(CatalogDocument::parse_persisted(&serde_json::to_vec(&document).unwrap()).is_ok());
}

#[test]
fn unresolvable_connects_to_and_bad_hashes_are_rejected() {
    let mut broken = document_json();
    broken["external_apps"] = serde_json::json!([]);
    let bytes = serde_json::to_vec(&broken).unwrap();
    assert!(matches!(
        CatalogDocument::parse(&bytes, now(), 0),
        Err(CatalogError::Invalid("connects_to"))
    ));

    let mut bad = document_json();
    bad["packages"][0]["archives"][0]["sha256"] = serde_json::json!("deadbeef");
    let bytes = serde_json::to_vec(&bad).unwrap();
    assert!(matches!(
        CatalogDocument::parse(&bytes, now(), 0),
        Err(CatalogError::Invalid("catalog entry"))
    ));

    let mut replay = document_json();
    replay["sequence"] = serde_json::json!(2);
    let bytes = serde_json::to_vec(&replay).unwrap();
    assert!(matches!(
        CatalogDocument::parse(&bytes, now(), 2),
        Err(CatalogError::Replay)
    ));

    let mut expired = document_json();
    expired["expires_at"] = serde_json::json!("2026-10-03T00:00:01Z");
    let bytes = serde_json::to_vec(&expired).unwrap();
    assert!(matches!(
        CatalogDocument::parse(&bytes, now(), 0),
        Err(CatalogError::Expired)
    ));
}
