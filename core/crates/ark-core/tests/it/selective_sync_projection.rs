#![allow(clippy::unwrap_used)]
use serde_json::json;

#[test]
fn frozen_filter_v1_is_canonical_and_bounded() {
    let typed = ark_domain::selective_sync::FilterV1::parse(
        r#"{"where":{"eq":{"value":null,"field":"title"}},"version":1}"#,
    )
    .unwrap();
    let reordered = ark_domain::selective_sync::FilterV1::parse(
        r#"{"version":1,"where":{"eq":{"field":"title","value":null}}}"#,
    )
    .unwrap();
    assert_eq!(typed, reordered);
    assert_eq!(typed.digest(), reordered.digest());
    assert_eq!(typed.digest().len(), 32);

    let canonical = ark_domain::selective_sync::validate_filter_json(
        r#"{"where":{"eq":{"value":null,"field":"title"}},"version":1}"#,
    )
    .unwrap();
    assert_eq!(
        canonical,
        r#"{"version":1,"where":{"eq":{"field":"title","value":null}}}"#
    );
    assert!(ark_domain::selective_sync::validate_filter_json(
        r#"{"version":1,"where":{"eq":{"field":"body","value":"x"},"bad":true}}"#
    )
    .is_err());
    assert!(ark_domain::selective_sync::validate_filter_json(
        r#"{"version":1,"where":{"in":{"field":"title","values":[]}}}"#
    )
    .is_err());
    assert_eq!(
        ark_domain::selective_sync::validate_filter_json("{}").unwrap(),
        r#"{"version":1,"where":{"and":[]}}"#
    );
    assert_eq!(
        ark_domain::selective_sync::filter_digest("{}").unwrap(),
        ark_domain::selective_sync::FilterV1::parse("{}")
            .unwrap()
            .digest()
    );
}

#[test]
fn filter_v1_evaluates_predicates_against_exact_entity_fields() {
    let fields = json!({"title":"Note", "priority":2, "enabled":true})
        .as_object()
        .unwrap()
        .clone();
    for predicate in [
        json!({"eq":{"field":"title","value":"Note"}}),
        json!({"in":{"field":"priority","values":[1,2,3]}}),
        json!({"exists":{"field":"enabled","value":true}}),
        json!({
            "and": [
                {"eq": {"field": "title", "value": "Note"}},
                {"not": {"eq": {"field": "priority", "value": 1}}}
            ]
        }),
        json!({
            "or": [
                {"eq": {"field": "title", "value": "Other"}},
                {"eq": {"field": "enabled", "value": true}}
            ]
        }),
    ] {
        let filter = serde_json::to_string(&json!({"version":1,"where":predicate})).unwrap();
        assert!(ark_domain::selective_sync::FilterV1::parse(&filter)
            .unwrap()
            .matches(&fields));
    }
    assert!(!ark_domain::selective_sync::FilterV1::parse(
        r#"{"version":1,"where":{"eq":{"field":"missing","value":null}}}"#
    )
    .unwrap()
    .matches(&fields));
    assert!(ark_domain::selective_sync::FilterV1::match_all().matches(&fields));
}

#[test]
fn selective_profile_projects_closure_in_order_and_keeps_local_copy_on_disable() {
    use ark_core::types::SyncEntity;
    let entity = |kind: &str, id: &str, data: serde_json::Value| SyncEntity {
        entity_type: kind.into(),
        id: id.into(),
        data: data.as_object().unwrap().clone(),
        hlc: format!("{kind}-{id}"),
        deleted: None,
        origin_device_id: None,
        origin_seq: None,
    };
    let entities = vec![
        entity(
            "object",
            "game",
            json!(
                {"typeId":"core.game",
                "typeVersion":"0.1.0",
                "title":"Game",
                "contentJson":{"body":"heavy"},
                "propsJson":{"localState":{"path":"C:/game"}}}),
        ),
        entity(
            "object",
            "note",
            json!(
                {"typeId":"core.note",
                "typeVersion":"0.1.0",
                "title":"Note",
                "contentJson":{"body":"full"},
                "propsJson":{}}),
        ),
        entity(
            "object_link",
            "link",
            json!({"sourceObjectId":"note","targetObjectId":"game","linkType":"related"}),
        ),
        entity(
            "usage_event",
            "usage-1",
            json!({"exePath":"C:/secret.exe","secret":"no"}),
        ),
        entity("object_type", "core.note", json!({"name":"Note"})),
    ];
    let mut profile = ark_domain::selective_sync::SelectiveSyncProfile::default();
    profile.set_rule(
        "type",
        "core.note",
        ark_domain::selective_sync::SyncMode::Full,
    );
    profile.set_rule(
        "type",
        "core.game",
        ark_domain::selective_sync::SyncMode::Metadata,
    );
    profile.set_rule(
        "dataset",
        "usage",
        ark_domain::selective_sync::SyncMode::None,
    );
    let projected = ark_domain::selective_sync::filter_entities_for_profile(&profile, &entities);
    assert_eq!(
        projected
            .iter()
            .map(|e| e.entity_type.as_str())
            .collect::<Vec<_>>(),
        ["object_type", "object", "object", "object_link"]
    );
    let game = projected.iter().find(|e| e.id == "game").unwrap();
    assert!(!game.data.contains_key("contentJson"));
    assert!(!game.data.contains_key("localState"));
    assert_eq!(game.data["propsJson"], json!({}));
    assert!(projected
        .iter()
        .all(|e| !e.data.contains_key("secret") && !e.data.contains_key("exePath")));
    profile.set_rule_with_filter(
        "type",
        "core.game",
        ark_domain::selective_sync::SyncMode::Metadata,
        ark_domain::selective_sync::FilterV1::parse(
            r#"{"version":1,"where":{"eq":{"field":"title","value":"Other"}}}"#,
        )
        .unwrap(),
    );
    let filtered = ark_domain::selective_sync::filter_entities_for_profile(&profile, &entities);
    assert_eq!(
        filtered
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<Vec<_>>(),
        ["core.note", "note"]
    );
    let disabled =
        ark_domain::selective_sync::filter_entities_for_profile(&Default::default(), &projected);
    assert!(disabled.is_empty());
    assert_eq!(projected.len(), 4);
}
