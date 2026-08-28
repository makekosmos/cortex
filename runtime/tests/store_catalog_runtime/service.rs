use super::*;

#[test]
fn external_url_requires_a_fresh_signed_external_listing() {
    let now = Utc.with_ymd_and_hms(2026, 8, 12, 1, 0, 0).unwrap();
    let key = SigningKey::from_bytes(&[12; 32]);
    let dir = tempfile::tempdir().unwrap();
    let service = StoreCatalogService::open(dir.path(), "store-key", key.verifying_key()).unwrap();
    service
        .apply(&signed(&document(1), "store-key", &key), now, &Index)
        .unwrap();
    assert_eq!(
        service.external_url_at("external.obsidian", now).unwrap(),
        "https://obsidian.md/"
    );
    assert!(matches!(
        service.external_url_at(
            "external.obsidian",
            Utc.with_ymd_and_hms(2026, 8, 14, 0, 0, 0).unwrap(),
        ),
        Err(TrustError::Unavailable)
    ));

    let empty =
        StoreCatalogService::open(dir.path().join("empty"), "store-key", key.verifying_key())
            .unwrap();
    assert!(matches!(
        empty.external_url_at("external.obsidian", now),
        Err(TrustError::Unavailable)
    ));

    let mut package = StoreListing::external("com.kosmos.eden", "Eden", "https://kosmos.local/");
    package.kind = ListingKind::KosmosPackage;
    package.distribution = Distribution::Package {
        package_id: "com.kosmos.eden".into(),
        version: "1.0.0".into(),
    };
    let package_doc = CatalogDocument {
        listings: vec![package],
        ..document(2)
    };
    let package_service =
        StoreCatalogService::open(dir.path().join("package"), "store-key", key.verifying_key())
            .unwrap();
    package_service
        .apply(&signed(&package_doc, "store-key", &key), now, &Index)
        .unwrap();
    assert!(matches!(
        package_service.external_url_at("com.kosmos.eden", now),
        Err(TrustError::Unavailable)
    ));
}

#[test]
fn persisted_catalog_replaces_atomically_on_windows() {
    let now = Utc.with_ymd_and_hms(2026, 8, 12, 1, 0, 0).unwrap();
    let key = SigningKey::from_bytes(&[13; 32]);
    let dir = tempfile::tempdir().unwrap();
    let service = StoreCatalogService::open(dir.path(), "store-key", key.verifying_key()).unwrap();
    service
        .apply(&signed(&document(1), "store-key", &key), now, &Index)
        .unwrap();
    service
        .apply(&signed(&document(2), "store-key", &key), now, &Index)
        .unwrap();

    let reopened = StoreCatalogService::open(dir.path(), "store-key", key.verifying_key()).unwrap();
    assert_eq!(reopened.catalog(now, vec![]).sequence, Some(2));
}
