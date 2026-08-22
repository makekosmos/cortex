#![allow(clippy::unwrap_used)]

use chrono::{TimeZone, Utc};
use ed25519_dalek::{Signer, SigningKey};
use kepler_backend::store_catalog::{
    CacheState, CatalogCache, CatalogDocument, Distribution, EffectiveGrantProjection,
    InstalledListing, ListingKind, PackageIndexLookup, Role, StoreCatalogEnvelope,
    StoreCatalogService, StoreCatalogTrust, StoreListing, TrustError,
};
use std::collections::BTreeSet;

struct Index;
impl PackageIndexLookup for Index {
    fn package_release(&self, package_id: &str, version: &str, is_bridge: bool) -> bool {
        matches!(
            (package_id, version, is_bridge),
            ("com.kosmos.eden", "1.0.0", false) | ("bridge", "1.0.0", true)
        )
    }

    fn canonical_type_version(&self, _type_id: &str, _versions: &str) -> bool {
        true
    }
}

#[test]
fn production_store_trust_is_embedded() {
    let directory = tempfile::tempdir().unwrap();
    StoreCatalogService::open_compiled(directory.path()).unwrap();
}

fn signed(document: &CatalogDocument, key_id: &str, key: &SigningKey) -> StoreCatalogEnvelope {
    let bytes = serde_json::to_vec(document).unwrap();
    StoreCatalogEnvelope::new(bytes.clone(), key_id, key.sign(&bytes).to_bytes().to_vec())
}

fn document(sequence: u64) -> CatalogDocument {
    CatalogDocument {
        schema_version: 1,
        sequence,
        issued_at: "2026-08-12T00:00:00Z".into(),
        expires_at: "2026-08-13T00:00:00Z".into(),
        listings: vec![StoreListing::external(
            "external.obsidian",
            "Obsidian",
            "https://obsidian.md/",
        )],
    }
}

#[test]
fn exact_bytes_and_active_store_key_are_required() {
    let key = SigningKey::from_bytes(&[7; 32]);
    let trust = StoreCatalogTrust::new("store-key", key.verifying_key()).unwrap();
    let envelope = signed(&document(1), "store-key", &key);
    let verified = trust
        .verify_at(
            &envelope,
            Utc.with_ymd_and_hms(2026, 8, 12, 1, 0, 0).unwrap(),
            &Index,
        )
        .unwrap();
    assert_eq!(verified.document.sequence, 1);

    let mut wrong = serde_json::to_vec(&document(2)).unwrap();
    let signature = key.sign(&wrong).to_bytes().to_vec();
    wrong.push(b' ');
    let bad = StoreCatalogEnvelope::new(wrong, "store-key", signature);
    assert!(matches!(
        trust.verify_at(&bad, Utc::now(), &Index),
        Err(TrustError::InvalidSignature)
    ));
}

#[test]
fn validation_rejects_unknown_fields_and_wrong_listing_invariants() {
    let key = SigningKey::from_bytes(&[8; 32]);
    let trust = StoreCatalogTrust::new("store-key", key.verifying_key()).unwrap();
    let mut value = serde_json::to_value(document(1)).unwrap();
    value["unexpected"] = true.into();
    let bytes = serde_json::to_vec(&value).unwrap();
    let envelope = StoreCatalogEnvelope::new(
        bytes.clone(),
        "store-key",
        key.sign(&bytes).to_bytes().to_vec(),
    );
    assert!(matches!(
        trust.verify_at(&envelope, Utc::now(), &Index),
        Err(TrustError::Json(_))
    ));
}

#[test]
fn cache_rejects_replay_and_preserves_last_verified_atomically() {
    let key = SigningKey::from_bytes(&[9; 32]);
    let trust = StoreCatalogTrust::new("store-key", key.verifying_key()).unwrap();
    let cache = CatalogCache::default();
    let now = Utc.with_ymd_and_hms(2026, 8, 12, 1, 0, 0).unwrap();
    let first = signed(&document(1), "store-key", &key);
    cache
        .apply(&trust.verify_at(&first, now, &Index).unwrap())
        .unwrap();
    assert_eq!(cache.state_at(now), CacheState::Fresh { sequence: 1 });
    assert!(matches!(
        cache.apply(&trust.verify_at(&first, now, &Index).unwrap()),
        Err(TrustError::Replay)
    ));
    assert_eq!(cache.sequence(), Some(1));
}

#[test]
fn expired_cache_preserves_last_value_but_reports_expired() {
    let key = SigningKey::from_bytes(&[10; 32]);
    let trust = StoreCatalogTrust::new("store-key", key.verifying_key()).unwrap();
    let cache = CatalogCache::default();
    let verified = trust
        .verify_at(
            &signed(&document(1), "store-key", &key),
            Utc.with_ymd_and_hms(2026, 8, 12, 1, 0, 0).unwrap(),
            &Index,
        )
        .unwrap();
    cache.apply(&verified).unwrap();
    assert_eq!(
        cache.state_at(Utc.with_ymd_and_hms(2026, 8, 14, 0, 0, 0).unwrap()),
        CacheState::Expired { sequence: 1 }
    );
    assert_eq!(cache.sequence(), Some(1));
}

#[test]
fn integration_requires_existing_external_listing() {
    let key = SigningKey::from_bytes(&[11; 32]);
    let trust = StoreCatalogTrust::new("store-key", key.verifying_key()).unwrap();
    let mut doc = document(1);
    let mut integration =
        StoreListing::external("integration.bridge", "Bridge", "https://bridge.example/");
    integration.kind = kepler_backend::store_catalog::ListingKind::Integration;
    integration.distribution = kepler_backend::store_catalog::Distribution::Integration {
        package_id: "bridge".into(),
        version: "1.0.0".into(),
        connects_to: "external.missing".into(),
    };
    integration.connects_to = Some("external.missing".into());
    doc.listings.push(integration);
    assert!(matches!(
        trust.verify_at(
            &signed(&doc, "store-key", &key),
            Utc.with_ymd_and_hms(2026, 8, 12, 1, 0, 0).unwrap(),
            &Index,
        ),
        Err(TrustError::Invalid("kind distribution"))
    ));
}

#[test]
fn catalog_dto_installed_projection_is_typed_and_contains_no_install_authority() {
    let cache = CatalogCache::default();
    let dto = cache.dto(
        Utc::now(),
        vec![InstalledListing {
            id: "com.kosmos.eden".into(),
            version: "1.0.0".into(),
            kind: "app".into(),
            enabled: true,
            revoked: false,
            publisher: "Kosmos".into(),
            effective_grants: vec![EffectiveGrantProjection {
                type_id: "com.kosmos.note".into(),
                version: "^1.0.0".into(),
                roles: BTreeSet::from([Role::Read, Role::Edit]),
                fields_read: vec!["title".into()],
                fields_write: vec!["title".into()],
                relations_read: vec![],
                relations_write: vec![],
            }],
        }],
    );
    let value = serde_json::to_value(dto).unwrap();
    assert_eq!(value["state"], "unavailable");
    assert_eq!(
        value["installed"][0]["effective_grants"][0]["type"],
        "com.kosmos.note"
    );
    assert_eq!(
        value["installed"][0]["effective_grants"][0]["roles"],
        serde_json::json!(["read", "edit"])
    );
    let payload = value.to_string();
    for forbidden in [
        "signature",
        "public_key",
        "archive_path",
        "token",
        "capabilities",
    ] {
        assert!(!payload.contains(forbidden), "store leaked {forbidden}");
    }
}

#[test]
fn listing_wire_values_match_manager_store_contract() {
    let value = serde_json::to_value(StoreListing::external(
        "external.obsidian",
        "Obsidian",
        "https://obsidian.md/",
    ))
    .unwrap();
    assert_eq!(value["kind"], "external-app");
    assert_eq!(value["publisher_tier"], "community");
    assert_eq!(
        value["distribution"]["official_url"],
        "https://obsidian.md/"
    );
}

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
