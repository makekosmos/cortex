#![allow(clippy::bool_assert_comparison, clippy::unwrap_used)]

use engine::runtime_grants::*;
use serde_json::json;

#[test]
fn typed_request_rejects_unknown_fields_and_duplicate_fields() {
    let unknown = serde_json::from_value::<DataRequest>(json!({
        "kind":"read_object", "type_id":"note", "type_version":"1.0.0",
        "object_id":"n1", "fields":[], "relations":[], "params":{}
    }));
    assert!(unknown.is_err());

    let duplicate = serde_json::from_value::<DataRequest>(json!({
        "kind":"read_object", "type_id":"note", "type_version":"1.0.0",
        "object_id":"n1", "fields":["title","title"], "relations":[]
    }));
    assert!(duplicate.is_err());
}

#[test]
fn compiler_intersects_duplicate_rules_and_projection_is_deterministic() {
    let registry = RegistrySnapshot::new(vec![RegisteredType::new(
        "note",
        "1.0.0",
        &["title", "body"],
        &["parent"],
    )]);
    let rules = vec![
        GrantRule::read("note", "1.0.0", &["title", "body"], &["parent"]),
        GrantRule::read("note", "1.0.0", &["title"], &["parent"]),
    ];
    let grant = GrantCompiler::compile(
        CompileInput::new("pkg", "1.0.0", "digest", rules),
        &registry,
    )
    .unwrap();
    assert_eq!(grant.rules[0].fields_read, vec!["title"]);
    assert_eq!(grant.rules[0].relations_read, vec!["parent"]);
    assert_eq!(
        grant.projection_json().unwrap(),
        grant.projection_json().unwrap()
    );
}

#[test]
fn compiler_accepts_a_version_range_matching_a_registered_definition() {
    let registry =
        RegistrySnapshot::new(vec![RegisteredType::new("note", "1.2.0", &["title"], &[])]);
    let grant = GrantCompiler::compile(
        CompileInput::new(
            "pkg",
            "1.0.0",
            "digest",
            vec![GrantRule::read("note", "^1.0.0", &["title"], &[])],
        ),
        &registry,
    );
    assert!(
        grant.is_ok(),
        "matching semver range should compile: {grant:?}"
    );
}

#[test]
fn grant_store_enforces_capacity_generation_and_revocation() {
    let clock = TestClock::new(100);
    let store = GrantStore::with_limits(clock.clone(), 1, 1);
    let first = store
        .mint(GrantIdentity::new("g1", "l1", "pkg", "1.0.0", "d", 0), 115)
        .unwrap();
    assert_eq!(
        store.mint(GrantIdentity::new("g2", "l2", "pkg", "1.0.0", "d", 0), 115),
        Err(GrantError::Capacity)
    );
    clock.set(115);
    assert_eq!(store.renew("l1", 0, 120), Err(GrantError::Expired));
    clock.set(110);
    let renewed = store.renew("l1", 0, 125).unwrap();
    assert_eq!(renewed.generation, 1);
    assert_eq!(store.renew("l1", 0, 125), Err(GrantError::StaleGeneration));
    assert_eq!(store.revoke("l1"), true);
    assert_eq!(store.revoke("l1"), false);
    assert_eq!(store.active_count(), 0);
    let _ = first;
}

#[test]
fn safe_projection_removes_capability_paths() {
    let p = SafeProjection::from_parts(
        "pkg",
        "1.0.0",
        "digest",
        vec![ScopedCapability::Storage {
            root_capability_id: "root".into(),
            actions: vec!["read".into()],
            path_patterns: vec!["/secret".into()],
            max_bytes: 4,
        }],
    );
    let value = p.to_value();
    assert_eq!(value["capabilities"][0]["root_capability_id"], "root");
    assert!(value["capabilities"][0].get("path_patterns").is_none());
}

#[test]
fn compiled_grant_denies_unregistered_typed_field_and_type() {
    let registry =
        RegistrySnapshot::new(vec![RegisteredType::new("note", "1.0.0", &["title"], &[])]);
    let grant = GrantCompiler::compile(
        CompileInput::new(
            "pkg",
            "1.0.0",
            "digest",
            vec![GrantRule {
                type_id: "note".into(),
                versions: vec!["1.0.0".into()],
                actions: ["read".into()].into_iter().collect(),
                fields_read: vec!["title".into()],
                fields_write: vec![],
                relations_read: vec![],
                relations_write: vec![],
            }],
        ),
        &registry,
    )
    .unwrap();
    assert!(grant
        .authorize_request(&DataRequest::ReadObject {
            type_id: "note".into(),
            type_version: "1.0.0".into(),
            object_id: "n".into(),
            fields: vec!["title".into()],
            relations: vec![],
        })
        .is_ok());
    assert!(grant
        .authorize_request(&DataRequest::ReadObject {
            type_id: "note".into(),
            type_version: "1.0.0".into(),
            object_id: "n".into(),
            fields: vec!["secret".into()],
            relations: vec![],
        })
        .is_err());
    assert!(grant
        .authorize_request(&DataRequest::ReadObject {
            type_id: "other".into(),
            type_version: "1.0.0".into(),
            object_id: "n".into(),
            fields: vec!["title".into()],
            relations: vec![],
        })
        .is_err());
}
